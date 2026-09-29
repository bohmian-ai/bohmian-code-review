//! PR-weighted paired hit@10 inference and the predeclared adoption rule.
//!
//! Each PR's arm score is its mean hit@10 over its own trials, so every PR
//! counts equally however many trials it contributes. An effect is the mean
//! per-PR difference of one arm against the caller's baseline arm with a
//! percentile bootstrap interval over PRs from [`BOOTSTRAP_SEED`]. The
//! decision arm is adopted only when every repository's decision − baseline
//! effect is [`Verdict::Met`].
//!
//! [`Evaluation`] is the reusable harness: it takes caller-named rankers
//! as [`Arms`], runs each sampled PR's trials, and summarizes them into
//! per-PR means, seeded effects, and the verdict.

use crate::trial::{Rankers, Trial, TrialError, evaluate};
use bohmian_code_review::{RelativePath, SymbolicContext};
use bohmian_code_review_pipeline::PullRequestFile;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Seed of every bootstrap, recorded in the report.
pub const BOOTSTRAP_SEED: u64 = 0x2026_0929_c0c4_a17e;
/// Bootstrap resamples over PRs.
pub const RESAMPLES: usize = 10_000;
/// Smallest F − C mean hit@10 difference that can adopt F: 5 points.
pub const ADOPTION_THRESHOLD: f64 = 0.05;
/// The predeclared decision rule as recorded in the report.
pub const DECISION_RULE: &str = "adopt F only if, in every repository, the mean per-PR F - C hit@10 difference is at least 0.05 and its 95% bootstrap interval's lower bound is above 0; an interval containing 0 is inconclusive";
/// Threats to validity every report states.
pub const VALIDITY_THREATS: [(&str, &str); 4] = [
    (
        "circular_label",
        "the label is co-change, and E predicts from earlier co-change, which favors E",
    ),
    (
        "one_repository_per_language",
        "results cannot separate language from codebase",
    ),
    (
        "proxy_for_relevance",
        "co-changed files stand in for the files a reviewer needed",
    ),
    (
        "small_sample",
        "50 PRs per repository detect only large effects",
    ),
];

/// One PR's mean hit@10 per arm over its own trials.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrMeans {
    /// PR number.
    pub pr: u64,
    /// Trials the means cover.
    pub trials: usize,
    /// Each arm's mean hit@10 by arm id.
    #[serde(flatten)]
    pub arms: BTreeMap<String, f64>,
}

impl PrMeans {
    /// Arm `id`'s mean hit@10; 0 for an arm no trial recorded.
    pub fn arm(&self, id: &str) -> f64 {
        self.arms.get(id).copied().unwrap_or_default()
    }
}

/// A paired treatment − baseline effect over PRs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Effect {
    /// Mean per-PR difference.
    pub mean: f64,
    /// 95% percentile bootstrap interval `[low, high]`.
    pub ci_95: [f64; 2],
}

/// One repository's decision − baseline outcome under [`DECISION_RULE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// At least [`ADOPTION_THRESHOLD`] with a lower bound above 0.
    Met,
    /// The interval contains 0.
    Inconclusive,
    /// Excludes 0 but misses the threshold or favors the baseline.
    NotMet,
}

/// Per-PR arm means of `trials`, in the order each PR first appears.
pub fn pr_means(trials: &[Trial]) -> Vec<PrMeans> {
    let mut means: Vec<PrMeans> = Vec::new();
    for trial in trials {
        let index = match means.iter().position(|means| means.pr == trial.pr) {
            Some(index) => index,
            None => {
                means.push(PrMeans {
                    pr: trial.pr,
                    trials: 0,
                    arms: BTreeMap::new(),
                });
                means.len() - 1
            }
        };
        let pr = &mut means[index];
        pr.trials += 1;
        for (id, outcome) in &trial.arms {
            *pr.arms.entry(id.clone()).or_default() += f64::from(u8::from(outcome.hit_at_10));
        }
    }
    for pr in &mut means {
        let trials = pr.trials as f64;
        for mean in pr.arms.values_mut() {
            *mean /= trials;
        }
    }
    means
}

/// The paired arm `treatment` − `baseline` effect over `means`,
/// bootstrapped from `seed`. No PRs is a zero effect.
pub fn effect(means: &[PrMeans], treatment: &str, baseline: &str, seed: u64) -> Effect {
    let differences: Vec<f64> = means
        .iter()
        .map(|pr| pr.arm(treatment) - pr.arm(baseline))
        .collect();
    let count = differences.len();
    if count == 0 {
        return Effect {
            mean: 0.0,
            ci_95: [0.0, 0.0],
        };
    }
    let mut rng = SplitMix64(seed);
    let mut resampled: Vec<f64> = (0..RESAMPLES)
        .map(|_| {
            (0..count)
                .map(|_| differences[(rng.next() % count as u64) as usize])
                .sum::<f64>()
                / count as f64
        })
        .collect();
    resampled.sort_by(f64::total_cmp);
    Effect {
        mean: differences.iter().sum::<f64>() / count as f64,
        ci_95: [
            resampled[RESAMPLES * 25 / 1000],
            resampled[RESAMPLES * 975 / 1000 - 1],
        ],
    }
}

/// The decision − baseline `effect`'s outcome under [`DECISION_RULE`].
pub fn verdict(effect: &Effect) -> Verdict {
    let [low, high] = effect.ci_95;
    if low <= 0.0 && high >= 0.0 {
        Verdict::Inconclusive
    } else if low > 0.0 && effect.mean >= ADOPTION_THRESHOLD {
        Verdict::Met
    } else {
        Verdict::NotMet
    }
}

/// Whether the decision arm is adopted: every repository in `verdicts` met
/// the rule.
pub fn adopt(verdicts: &[Verdict]) -> bool {
    !verdicts.is_empty() && verdicts.iter().all(|verdict| *verdict == Verdict::Met)
}

/// Why caller-named arms cannot be evaluated.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArmsError {
    /// Two rankers share one id, so their outcomes would merge.
    #[error("arm `{0}` is named twice")]
    Duplicate(String),
    /// The baseline or decision arm names no ranker.
    #[error("arm `{0}` names no ranker")]
    Missing(String),
}

/// Caller-named rankers with the baseline every effect pairs against and
/// the arm the adoption decision is about.
///
/// Invariant: ids are distinct and both `baseline` and `decision` name one
/// of `rankers`.
#[derive(Clone, Copy)]
pub struct Arms<'a> {
    rankers: &'a Rankers<'a>,
    baseline: &'a str,
    decision: &'a str,
}

impl<'a> Arms<'a> {
    /// Validate `rankers` with their `baseline` and `decision` arms.
    ///
    /// # Errors
    ///
    /// [`ArmsError::Duplicate`] for a repeated id and
    /// [`ArmsError::Missing`] when `baseline` or `decision` names no ranker.
    pub fn new(
        rankers: &'a Rankers<'a>,
        baseline: &'a str,
        decision: &'a str,
    ) -> Result<Arms<'a>, ArmsError> {
        let mut ids = BTreeSet::new();
        for (id, _) in rankers {
            if !ids.insert(*id) {
                return Err(ArmsError::Duplicate((*id).to_owned()));
            }
        }
        for arm in [baseline, decision] {
            if !ids.contains(arm) {
                return Err(ArmsError::Missing(arm.to_owned()));
            }
        }
        Ok(Arms {
            rankers,
            baseline,
            decision,
        })
    }

    /// The baseline arm's id.
    pub fn baseline(&self) -> &'a str {
        self.baseline
    }

    /// The decision arm's id.
    pub fn decision(&self) -> &'a str {
        self.decision
    }
}

/// One repository sample's trials under [`Arms`], accumulated PR by PR so
/// each PR's source leaves memory once its trials ran.
pub struct Evaluation<'a> {
    arms: Arms<'a>,
    trials: Vec<Trial>,
}

/// One repository sample's evaluated result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// Every trial in the order PRs were added.
    pub trials: Vec<Trial>,
    /// Per-PR arm means, in the order each PR first appears.
    pub means: Vec<PrMeans>,
    /// Each non-baseline arm's effect by `"<arm>-<baseline>"`, from
    /// [`BOOTSTRAP_SEED`].
    pub effects: BTreeMap<String, Effect>,
    /// The decision − baseline verdict.
    pub verdict: Verdict,
}

impl<'a> Evaluation<'a> {
    /// An empty evaluation under `arms`.
    pub fn new(arms: Arms<'a>) -> Evaluation<'a> {
        Evaluation {
            arms,
            trials: Vec::new(),
        }
    }

    /// Run sampled PR `pr`'s trials, as [`evaluate`] does, under every arm.
    ///
    /// # Errors
    ///
    /// The errors of [`evaluate`]; no trial of `pr` is kept on error.
    pub fn add(
        &mut self,
        pr: u64,
        index: &SymbolicContext,
        listed: &[PullRequestFile],
        heads: &BTreeMap<RelativePath, &[u8]>,
        history: &[&[RelativePath]],
    ) -> Result<(), TrialError> {
        let trials = evaluate(pr, index, listed, heads, history, self.arms.rankers)?;
        self.trials.extend(trials);
        Ok(())
    }

    /// The [`Summary`] of the trials added.
    pub fn finish(self) -> Summary {
        Summary::new(self.trials, self.arms)
    }
}

impl Summary {
    /// Per-PR means, every non-baseline arm's seeded effect, and the
    /// decision verdict of `trials` under `arms`.
    pub fn new(trials: Vec<Trial>, arms: Arms<'_>) -> Summary {
        let means = pr_means(&trials);
        let baseline = arms.baseline;
        let effects = arms
            .rankers
            .iter()
            .filter(|(id, _)| *id != baseline)
            .map(|(id, _)| {
                (
                    format!("{id}-{baseline}"),
                    effect(&means, id, baseline, BOOTSTRAP_SEED),
                )
            })
            .collect();
        let verdict = verdict(&effect(&means, arms.decision, baseline, BOOTSTRAP_SEED));
        Summary {
            trials,
            means,
            effects,
            verdict,
        }
    }
}

/// SplitMix64, a fixed seeded generator so a rerun resamples identically.
struct SplitMix64(u64);

impl SplitMix64 {
    /// The next 64-bit output.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Inference checks over hand-built trials.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trial::Outcome;
    use bohmian_code_review::{CoChange, DEFAULT_RANKER, Ranker, RaritySymbol, RelativePath};
    use serde_json::Value;

    /// A trial of PR `pr` whose C, E, and F hit@10 are `hits`.
    fn trial(pr: u64, hits: [bool; 3]) -> Trial {
        let outcome = |hit: bool| Outcome {
            rank: if hit { Some(1) } else { None },
            hit_at_10: hit,
        };
        Trial {
            pr,
            withheld: RelativePath::new("src/a.py").expect("fixture path is canonical"),
            arms: BTreeMap::from([
                ("C".to_owned(), outcome(hits[0])),
                ("E".to_owned(), outcome(hits[1])),
                ("F".to_owned(), outcome(hits[2])),
            ]),
        }
    }

    /// PR means with a fixed F − C per-PR difference for each entry.
    fn differences(values: &[f64]) -> Vec<PrMeans> {
        values
            .iter()
            .enumerate()
            .map(|(pr, value)| PrMeans {
                pr: pr as u64,
                trials: 1,
                arms: BTreeMap::from([
                    ("C".to_owned(), 0.0),
                    ("E".to_owned(), 0.0),
                    ("F".to_owned(), *value),
                ]),
            })
            .collect()
    }

    /// Unequal trial counts still weight PRs equally: PR 1's three trials
    /// and PR 2's one trial each contribute one paired difference, so F − C
    /// is (1/3 + 1) / 2, not the all-trial 2/4.
    #[test]
    fn prs_weigh_equally_whatever_their_trial_counts() {
        let trials = [
            trial(1, [false, true, true]),
            trial(1, [false, false, false]),
            trial(2, [false, true, true]),
            trial(1, [true, true, true]),
        ];
        let means = pr_means(&trials);
        assert_eq!(
            means
                .iter()
                .map(|pr| (pr.pr, pr.trials))
                .collect::<Vec<_>>(),
            [(1, 3), (2, 1)]
        );
        assert_eq!(
            (means[0].arm("C"), means[0].arm("E"), means[0].arm("F")),
            (1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0)
        );
        let f_minus_c = effect(&means, "F", "C", BOOTSTRAP_SEED);
        assert_eq!(f_minus_c.mean, (1.0 / 3.0 + 1.0) / 2.0);
        assert_eq!(f_minus_c.ci_95, [1.0 / 3.0, 1.0]);
        assert!(pr_means(&[]).is_empty());
    }

    /// A fixed seed resamples identically to a pinned interval around the
    /// mean; a constant difference has a degenerate interval.
    #[test]
    fn bootstrap_is_fixed_by_its_seed() {
        let means = differences(&[0.0, 0.2, 0.4, 1.0, -0.6, 0.8, 0.0, 0.2]);
        let first = effect(&means, "F", "C", BOOTSTRAP_SEED);
        assert_eq!(first, effect(&means, "F", "C", BOOTSTRAP_SEED));
        assert_eq!(first.ci_95, [-0.07499999999999998, 0.575]);
        assert!(first.ci_95[0] < first.mean && first.mean < first.ci_95[1]);
        assert_eq!(
            effect(&differences(&[0.25; 50]), "F", "C", BOOTSTRAP_SEED),
            Effect {
                mean: 0.25,
                ci_95: [0.25, 0.25]
            }
        );
        assert_eq!(
            effect(&[], "F", "C", BOOTSTRAP_SEED),
            Effect {
                mean: 0.0,
                ci_95: [0.0, 0.0]
            }
        );
    }

    /// Differences of 0 and 1 resample to means of 0, 0.5, or 1 with
    /// probabilities 1/4, 1/2, 1/4, so the interval is exactly [0, 1] and
    /// contains 0: inconclusive, and F is not adopted.
    #[test]
    fn a_zero_containing_interval_is_inconclusive() {
        let f_minus_c = effect(&differences(&[0.0, 1.0]), "F", "C", BOOTSTRAP_SEED);
        assert_eq!(
            f_minus_c,
            Effect {
                mean: 0.5,
                ci_95: [0.0, 1.0]
            }
        );
        assert_eq!(verdict(&f_minus_c), Verdict::Inconclusive);
        assert!(!adopt(&[Verdict::Met, verdict(&f_minus_c), Verdict::Met]));
    }

    /// F is adopted only when every repository meets both the threshold
    /// and a lower bound above 0.
    #[test]
    fn adoption_needs_every_repository_to_meet_the_rule() {
        let met = Effect {
            mean: 0.05,
            ci_95: [0.01, 0.1],
        };
        let small = Effect {
            mean: 0.04,
            ci_95: [0.01, 0.1],
        };
        let worse = Effect {
            mean: -0.2,
            ci_95: [-0.3, -0.1],
        };
        assert_eq!(
            [verdict(&met), verdict(&small), verdict(&worse)],
            [Verdict::Met, Verdict::NotMet, Verdict::NotMet]
        );
        assert!(adopt(&[Verdict::Met; 3]));
        assert!(!adopt(&[Verdict::Met, Verdict::NotMet, Verdict::Met]));
        assert!(!adopt(&[]));
    }

    /// Report regression: each checked-in C/E/F report's trials, summarized
    /// under the caller-named C/E/F arms, reproduce its per-PR means, both
    /// seeded effects bit for bit, its verdict, and its trial records.
    #[test]
    fn checked_in_reports_are_reproduced_from_their_trials() {
        let rankers: [(&str, &dyn Ranker); 3] = [
            ("C", &RaritySymbol),
            ("E", &CoChange),
            ("F", &DEFAULT_RANKER),
        ];
        let arms = Arms::new(&rankers, "C", "F").expect("valid arms");
        for text in [
            include_str!("../../../changes/completed/2026/cochange-eval-reports/pydantic-ai.json"),
            include_str!("../../../changes/completed/2026/cochange-eval-reports/tokio.json"),
            include_str!("../../../changes/completed/2026/cochange-eval-reports/zod.json"),
        ] {
            let report: Value = serde_json::from_str(text).expect("report is JSON");
            let records = report["trials"].as_array().expect("trials");
            let trials: Vec<Trial> = records
                .iter()
                .map(|record| Trial {
                    pr: record["pr"].as_u64().expect("pr"),
                    withheld: RelativePath::new(record["withheld"].as_str().expect("withheld"))
                        .expect("canonical path"),
                    arms: ["C", "E", "F"]
                        .map(|id| {
                            let arm = &record[id];
                            let outcome = Outcome {
                                rank: arm["rank"].as_u64().map(|rank| rank as usize),
                                hit_at_10: arm["hit_at_10"].as_bool().expect("hit"),
                            };
                            (id.to_owned(), outcome)
                        })
                        .into(),
                })
                .collect();
            let summary = Summary::new(trials, arms);

            assert_eq!(
                serde_json::to_value(&summary.trials).expect("serializes"),
                report["trials"]
            );
            let sampled: Vec<&Value> = report["sample"]
                .as_array()
                .expect("sample")
                .iter()
                .map(|pr| &pr["hit_at_10_means"])
                .filter(|means| !means.is_null())
                .collect();
            assert_eq!(
                summary
                    .means
                    .iter()
                    .map(|means| serde_json::to_value(means).expect("serializes"))
                    .collect::<Vec<_>>(),
                sampled.into_iter().cloned().collect::<Vec<_>>()
            );
            for (name, effect) in &summary.effects {
                // Both sides go through the same text parse, so equal
                // shortest float text compares equal bit for bit.
                let text = serde_json::to_string(effect).expect("serializes");
                let computed: Value = serde_json::from_str(&text).expect("reparses");
                let reported = &report["effects"][name];
                assert_eq!(computed["mean"], reported["mean"], "{name}");
                assert_eq!(computed["ci_95"], reported["ci_95"], "{name}");
            }
            assert_eq!(summary.effects.keys().collect::<Vec<_>>(), ["E-C", "F-C"]);
            assert_eq!(
                serde_json::to_value(summary.verdict).expect("serializes"),
                report["decision"]["repository_f_minus_c"]
            );
        }
    }
}

/// Consumer journey of the harness through its public API only: a caller
/// supplies its own small repository sample and its own named rankers, and
/// reads each arm's hit@10 and the decision back by name, without the live
/// command.
#[cfg(test)]
mod consumer_tests {
    use crate::report::{Arms, ArmsError, Evaluation, Verdict};
    use bohmian_code_review::{
        RankInput, RankedFile, Ranker, RaritySymbol, RelativePath, SymbolicContext,
    };
    use bohmian_code_review_pipeline::{FileStatus, ObjectSha, PullRequestFile};
    use std::collections::BTreeMap;

    /// A caller ranker that ranks the fixed `paths` in order, whatever changed.
    struct Fixed(&'static [&'static str]);

    impl Ranker for Fixed {
        /// Ranks every fixed path with descending scores.
        fn rank(&self, _input: &RankInput<'_>) -> Vec<RankedFile> {
            self.0
                .iter()
                .enumerate()
                .map(|(index, raw)| RankedFile {
                    path: path(raw),
                    score: 1.0 / (index as f64 + 1.0),
                    reasons: Vec::new(),
                })
                .collect()
        }
    }

    /// Canonical fixture path.
    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("fixture path is canonical")
    }

    /// The base index as a snapshot read-back builds it.
    fn base_index(files: &[(&str, &str)]) -> SymbolicContext {
        let mut extracted = SymbolicContext::new();
        for (raw, text) in files {
            extracted
                .upsert(raw, text.as_bytes())
                .expect("fixture extracts");
        }
        let mut index = SymbolicContext::new();
        for facts in extracted.export() {
            index.load(facts).expect("exported facts load");
        }
        index
    }

    /// A modified listing entry changing line 2.
    fn modified(raw: &str) -> PullRequestFile {
        PullRequestFile {
            path: path(raw),
            status: FileStatus::Modified,
            sha: ObjectSha::new(&"c".repeat(40)).expect("fixture sha is valid"),
            patch: Some("@@ -2 +2 @@\n-    return 0\n+    return 1".to_owned()),
            previous_path: None,
        }
    }

    /// Two PRs, each modifying two declared files, evaluated under a caller's
    /// `oracle` ranker and a caller's `silent` baseline: every trial is a hit
    /// for `oracle` and a miss for `silent`, so the per-PR means, the named
    /// `oracle-silent` effect, and the decision all come back by the caller's
    /// names, and the decision is met.
    #[test]
    fn caller_sample_and_named_rankers_produce_named_results() {
        let index = base_index(&[
            ("pkg/a.py", "def a():\n    return 0\n"),
            ("pkg/b.py", "def b():\n    return 0\n"),
            ("pkg/c.py", "def c():\n    return 0\n"),
        ]);
        let heads_owned: BTreeMap<RelativePath, String> = ["a", "b", "c"]
            .iter()
            .map(|name| {
                (
                    path(&format!("pkg/{name}.py")),
                    format!("def {name}():\n    return 1\n"),
                )
            })
            .collect();
        let heads: BTreeMap<RelativePath, &[u8]> = heads_owned
            .iter()
            .map(|(path, text)| (path.clone(), text.as_bytes()))
            .collect();
        let oracle = Fixed(&["pkg/a.py", "pkg/b.py", "pkg/c.py"]);
        let silent = Fixed(&[]);
        let rankers: [(&str, &dyn Ranker); 3] = [
            ("silent", &silent),
            ("oracle", &oracle),
            ("rarity", &RaritySymbol),
        ];
        let arms = Arms::new(&rankers, "silent", "oracle").expect("valid arms");

        let mut evaluation = Evaluation::new(arms);
        for (pr, files) in [(1, ["pkg/a.py", "pkg/b.py"]), (2, ["pkg/b.py", "pkg/c.py"])] {
            let listed: Vec<PullRequestFile> = files.iter().map(|raw| modified(raw)).collect();
            evaluation
                .add(pr, &index, &listed, &heads, &[])
                .expect("trials run");
        }
        let summary = evaluation.finish();

        assert_eq!(summary.trials.len(), 4);
        for trial in &summary.trials {
            assert!(trial.arms["oracle"].hit_at_10, "{trial:?}");
            assert!(!trial.arms["silent"].hit_at_10, "{trial:?}");
        }
        assert_eq!(
            summary.means.iter().map(|pr| pr.pr).collect::<Vec<_>>(),
            [1, 2]
        );
        for pr in &summary.means {
            assert_eq!((pr.arm("oracle"), pr.arm("silent")), (1.0, 0.0));
        }
        assert_eq!(
            summary.effects.keys().collect::<Vec<_>>(),
            ["oracle-silent", "rarity-silent"]
        );
        let effect = summary.effects["oracle-silent"];
        assert_eq!((effect.mean, effect.ci_95), (1.0, [1.0, 1.0]));
        assert_eq!(summary.verdict, Verdict::Met);

        let trial = serde_json::to_value(&summary.trials[0]).expect("serializes");
        assert_eq!(trial["oracle"]["hit_at_10"], true);
        assert_eq!(trial["silent"]["rank"], "unranked");
    }

    /// Arms that could not be told apart or name no ranker are refused before
    /// any trial runs.
    #[test]
    fn ambiguous_or_missing_arms_are_refused() {
        let one = Fixed(&[]);
        let twice: [(&str, &dyn Ranker); 2] = [("x", &one), ("x", &RaritySymbol)];
        assert_eq!(
            Arms::new(&twice, "x", "x").err(),
            Some(ArmsError::Duplicate("x".to_owned()))
        );
        let single: [(&str, &dyn Ranker); 1] = [("x", &one)];
        assert_eq!(
            Arms::new(&single, "x", "y").err(),
            Some(ArmsError::Missing("y".to_owned()))
        );
    }
}
