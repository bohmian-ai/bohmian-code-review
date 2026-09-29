//! Per-arm hit@10 over a repository's paired trials.

use crate::trial::Trial;
use serde::Serialize;
use std::collections::BTreeMap;

/// One arm's hit@10 over a set of trials.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArmSummary {
    /// Trials aggregated.
    pub trials: usize,
    /// Share of trials ranking the withheld file 10th or better.
    pub hit_at_10: f64,
}

/// Each arm's hit@10 over `trials`.
pub fn summarize(trials: &[Trial]) -> BTreeMap<&'static str, ArmSummary> {
    let mut summaries = BTreeMap::new();
    for arm in 0..3 {
        let hits = trials
            .iter()
            .filter(|trial| trial.arms()[arm].1.hit_at_10)
            .count();
        summaries.insert(
            ["C", "E", "F"][arm],
            ArmSummary {
                trials: trials.len(),
                hit_at_10: match trials.len() {
                    0 => 0.0,
                    total => hits as f64 / total as f64,
                },
            },
        );
    }
    summaries
}

/// Aggregate checks over hand-built trials.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trial::Outcome;
    use bohmian_code_review::RelativePath;

    /// A trial of PR `pr` with C, E, and F at `ranks`.
    fn trial(pr: u64, ranks: [Option<usize>; 3]) -> Trial {
        let outcome = |rank: Option<usize>| Outcome {
            rank,
            hit_at_10: rank.is_some_and(|rank| rank <= 10),
        };
        Trial {
            pr,
            withheld: RelativePath::new("src/a.py").expect("fixture path is canonical"),
            c: outcome(ranks[0]),
            e: outcome(ranks[1]),
            f: outcome(ranks[2]),
        }
    }

    /// Each arm's hit@10 share counts only its own hits; no trials is 0.
    #[test]
    fn summaries_follow_each_arms_hits() {
        let trials = [
            trial(1, [Some(1), None, Some(10)]),
            trial(1, [Some(11), Some(2), Some(3)]),
        ];
        let summary = summarize(&trials);
        assert_eq!(summary["C"].hit_at_10, 0.5);
        assert_eq!(summary["E"].hit_at_10, 0.5);
        assert_eq!(summary["F"].hit_at_10, 1.0);
        assert_eq!(summary["F"].trials, 2);
        assert_eq!(summarize(&[])["C"].hit_at_10, 0.0);
    }
}
