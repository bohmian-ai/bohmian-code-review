//! PR-weighted paired hit@10 inference and the predeclared adoption rule.
//!
//! Each PR's arm score is its mean hit@10 over its own trials, so every PR
//! counts equally however many trials it contributes. An effect is the mean
//! per-PR difference against C with a percentile bootstrap interval over
//! PRs from [`BOOTSTRAP_SEED`]. F is adopted only when every repository's
//! F − C effect is [`Verdict::Met`].

use crate::trial::Trial;
use serde::Serialize;

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
    /// C's mean hit@10.
    #[serde(rename = "C")]
    pub c: f64,
    /// E's mean hit@10.
    #[serde(rename = "E")]
    pub e: f64,
    /// F's mean hit@10.
    #[serde(rename = "F")]
    pub f: f64,
}

/// A paired treatment − C effect over PRs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Effect {
    /// Mean per-PR difference.
    pub mean: f64,
    /// 95% percentile bootstrap interval `[low, high]`.
    pub ci_95: [f64; 2],
}

/// One repository's F − C outcome under [`DECISION_RULE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// At least [`ADOPTION_THRESHOLD`] with a lower bound above 0.
    Met,
    /// The interval contains 0.
    Inconclusive,
    /// Excludes 0 but misses the threshold or favors C.
    NotMet,
}

/// Per-PR arm means of `trials`, in the order each PR first appears.
pub fn pr_means(trials: &[Trial]) -> Vec<PrMeans> {
    let mut means: Vec<PrMeans> = Vec::new();
    for trial in trials {
        let hits = trial
            .arms()
            .map(|(_, outcome)| f64::from(u8::from(outcome.hit_at_10)));
        match means.iter_mut().find(|means| means.pr == trial.pr) {
            Some(pr) => {
                pr.trials += 1;
                pr.c += hits[0];
                pr.e += hits[1];
                pr.f += hits[2];
            }
            None => means.push(PrMeans {
                pr: trial.pr,
                trials: 1,
                c: hits[0],
                e: hits[1],
                f: hits[2],
            }),
        }
    }
    for pr in &mut means {
        let trials = pr.trials as f64;
        pr.c /= trials;
        pr.e /= trials;
        pr.f /= trials;
    }
    means
}

/// The paired `treatment` − C effect over `means`, bootstrapped from
/// `seed`. No PRs is a zero effect.
pub fn effect(means: &[PrMeans], treatment: fn(&PrMeans) -> f64, seed: u64) -> Effect {
    let differences: Vec<f64> = means.iter().map(|pr| treatment(pr) - pr.c).collect();
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

/// `f_minus_c`'s outcome under [`DECISION_RULE`].
pub fn verdict(f_minus_c: &Effect) -> Verdict {
    let [low, high] = f_minus_c.ci_95;
    if low <= 0.0 && high >= 0.0 {
        Verdict::Inconclusive
    } else if low > 0.0 && f_minus_c.mean >= ADOPTION_THRESHOLD {
        Verdict::Met
    } else {
        Verdict::NotMet
    }
}

/// Whether F is adopted: every repository in `verdicts` met the rule.
pub fn adopt(verdicts: &[Verdict]) -> bool {
    !verdicts.is_empty() && verdicts.iter().all(|verdict| *verdict == Verdict::Met)
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
    use bohmian_code_review::RelativePath;

    /// A trial of PR `pr` whose C, E, and F hit@10 are `hits`.
    fn trial(pr: u64, hits: [bool; 3]) -> Trial {
        let outcome = |hit: bool| Outcome {
            rank: if hit { Some(1) } else { None },
            hit_at_10: hit,
        };
        Trial {
            pr,
            withheld: RelativePath::new("src/a.py").expect("fixture path is canonical"),
            c: outcome(hits[0]),
            e: outcome(hits[1]),
            f: outcome(hits[2]),
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
                c: 0.0,
                e: 0.0,
                f: *value,
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
            (means[0].c, means[0].e, means[0].f),
            (1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0)
        );
        let f_minus_c = effect(&means, |pr| pr.f, BOOTSTRAP_SEED);
        assert_eq!(f_minus_c.mean, (1.0 / 3.0 + 1.0) / 2.0);
        assert_eq!(f_minus_c.ci_95, [1.0 / 3.0, 1.0]);
        assert!(pr_means(&[]).is_empty());
    }

    /// A fixed seed resamples identically to a pinned interval around the
    /// mean; a constant difference has a degenerate interval.
    #[test]
    fn bootstrap_is_fixed_by_its_seed() {
        let means = differences(&[0.0, 0.2, 0.4, 1.0, -0.6, 0.8, 0.0, 0.2]);
        let first = effect(&means, |pr| pr.f, BOOTSTRAP_SEED);
        assert_eq!(first, effect(&means, |pr| pr.f, BOOTSTRAP_SEED));
        assert_eq!(first.ci_95, [-0.07499999999999998, 0.575]);
        assert!(first.ci_95[0] < first.mean && first.mean < first.ci_95[1]);
        assert_eq!(
            effect(&differences(&[0.25; 50]), |pr| pr.f, BOOTSTRAP_SEED),
            Effect {
                mean: 0.25,
                ci_95: [0.25, 0.25]
            }
        );
        assert_eq!(
            effect(&[], |pr| pr.f, BOOTSTRAP_SEED),
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
        let f_minus_c = effect(&differences(&[0.0, 1.0]), |pr| pr.f, BOOTSTRAP_SEED);
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
}
