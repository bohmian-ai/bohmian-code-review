//! Per-variant aggregates and misses over a repository's trials.
//!
//! A trial is a hit for a variant when the common experimental packing
//! sends the withheld file whole or as spans; otherwise, unranked or
//! omitted, it is a miss.

use crate::trial::{Outcome, Sent, Trial};
use bohmian_code_review::RelativePath;
use serde::Serialize;
use std::collections::BTreeMap;

/// Ranks at which hit rate is reported.
const HIT_RANKS: [usize; 4] = [1, 5, 10, 50];

/// One variant's aggregates over a set of trials.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VariantSummary {
    /// Trials aggregated.
    pub trials: usize,
    /// Share of trials ranking the withheld file at or above 1, 5, 10, 50.
    pub hit_rate_at: BTreeMap<usize, f64>,
    /// Share of trials leaving it unranked.
    pub unranked: f64,
    /// Share of trials sending it whole.
    pub sent_whole: f64,
    /// Share of trials sending it as spans only.
    pub sent_spans_only: f64,
    /// Ascending withheld-file scores of hit trials.
    pub hit_scores: Vec<f64>,
    /// Ascending withheld-file scores of miss trials; zero when unranked.
    pub miss_scores: Vec<f64>,
}

/// A trial whose withheld file a variant did not send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Miss {
    /// PR number.
    pub pr: u64,
    /// The withheld file.
    pub path: RelativePath,
}

/// Whether `outcome` sent the withheld file.
fn is_hit(outcome: &Outcome) -> bool {
    matches!(outcome.sent, Sent::Whole | Sent::SpansOnly)
}

/// Each variant's aggregates over `trials`.
pub fn summarize<'t>(
    trials: impl Iterator<Item = &'t Trial> + Clone,
) -> BTreeMap<&'static str, VariantSummary> {
    let mut summaries = BTreeMap::new();
    for variant in 0..4 {
        let outcomes: Vec<&Outcome> = trials
            .clone()
            .map(|trial| trial.variants()[variant].1)
            .collect();
        let share = |matches: &dyn Fn(&Outcome) -> bool| match outcomes.len() {
            0 => 0.0,
            total => {
                outcomes.iter().filter(|outcome| matches(outcome)).count() as f64 / total as f64
            }
        };
        let scores = |hit: bool| {
            let mut scores: Vec<f64> = outcomes
                .iter()
                .filter(|outcome| is_hit(outcome) == hit)
                .map(|outcome| outcome.score)
                .collect();
            scores.sort_by(f64::total_cmp);
            scores
        };
        let id = ["A", "B", "C", "D"][variant];
        summaries.insert(
            id,
            VariantSummary {
                trials: outcomes.len(),
                hit_rate_at: HIT_RANKS
                    .iter()
                    .map(|&k| {
                        (
                            k,
                            share(&|outcome| outcome.rank.is_some_and(|rank| rank <= k)),
                        )
                    })
                    .collect(),
                unranked: share(&|outcome| outcome.sent == Sent::Unranked),
                sent_whole: share(&|outcome| outcome.sent == Sent::Whole),
                sent_spans_only: share(&|outcome| outcome.sent == Sent::SpansOnly),
                hit_scores: scores(true),
                miss_scores: scores(false),
            },
        );
    }
    summaries
}

/// Each variant's misses over `trials`, in trial order.
pub fn misses(trials: &[Trial]) -> BTreeMap<&'static str, Vec<Miss>> {
    let mut misses: BTreeMap<&'static str, Vec<Miss>> = BTreeMap::new();
    for trial in trials {
        for (id, outcome) in trial.variants() {
            let missed = misses.entry(id).or_default();
            if !is_hit(outcome) {
                missed.push(Miss {
                    pr: trial.pr,
                    path: trial.withheld.clone(),
                });
            }
        }
    }
    misses
}

/// Aggregate checks over hand-built trials.
#[cfg(test)]
mod tests {
    use super::*;

    /// An outcome at `rank` with `score` and `sent`.
    fn outcome(rank: Option<usize>, score: f64, sent: Sent) -> Outcome {
        Outcome {
            rank,
            score,
            names: Vec::new(),
            direction: None,
            whole_tokens: 1,
            snippet_tokens: 0,
            sent,
        }
    }

    /// A trial of PR `pr` whose every variant has `outcome`.
    fn trial(pr: u64, raw: &str, outcome: Outcome) -> Trial {
        Trial {
            pr,
            withheld: RelativePath::new(raw).expect("fixture path is canonical"),
            test_file: raw.starts_with("tests/"),
            diff_tokens: 0,
            omitted: Vec::new(),
            excluded: Vec::new(),
            a: outcome.clone(),
            b: outcome.clone(),
            c: outcome.clone(),
            d: outcome,
            production_selected: false,
            names_scoped: 0,
            names_unscoped: 0,
        }
    }

    /// Hit rates, shares, score distributions, and misses follow the sent
    /// outcomes, and the non-test split drops test paths.
    #[test]
    fn summaries_and_misses_follow_outcomes() {
        let trials = [
            trial(1, "src/a.py", outcome(Some(1), 2.0, Sent::Whole)),
            trial(1, "src/b.py", outcome(Some(7), 0.5, Sent::SpansOnly)),
            trial(2, "src/c.py", outcome(Some(60), 0.25, Sent::Omitted)),
            trial(2, "tests/test_d.py", outcome(None, 0.0, Sent::Unranked)),
        ];
        let all = &summarize(trials.iter())["C"];
        assert_eq!(all.trials, 4);
        assert_eq!(all.hit_rate_at[&1], 0.25);
        assert_eq!(all.hit_rate_at[&10], 0.5);
        assert_eq!(all.hit_rate_at[&50], 0.5);
        assert_eq!(
            (all.unranked, all.sent_whole, all.sent_spans_only),
            (0.25, 0.25, 0.25)
        );
        assert_eq!(all.hit_scores, [0.5, 2.0]);
        assert_eq!(all.miss_scores, [0.0, 0.25]);
        let non_test = &summarize(trials.iter().filter(|trial| !trial.test_file))["A"];
        assert_eq!(non_test.trials, 3);
        assert_eq!(non_test.unranked, 0.0);
        let misses = misses(&trials);
        assert_eq!(
            misses["D"]
                .iter()
                .map(|miss| (miss.pr, miss.path.as_str()))
                .collect::<Vec<_>>(),
            [(2, "src/c.py"), (2, "tests/test_d.py")]
        );
    }
}
