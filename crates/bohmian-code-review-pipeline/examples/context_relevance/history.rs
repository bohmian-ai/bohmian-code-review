//! Time-eligible co-change history for the co-change experiment.
//!
//! A PR is merged before a commit when its merge time is earlier than the
//! commit's committer time. The cold build point is the earliest sampled
//! base commit time; cold history is the [`COLD_DEPTH`] most recent
//! non-mass-edit PRs merged before it, and every non-mass-edit PR merged
//! from it up to the newest sampled base extends history incrementally.
//! [`History::build`] reads each needed PR's file listing once, through the
//! newest base only, and a trial filters that one history by its own base
//! time without any provider read. A PR changing more than
//! [`MASS_EDIT_FILES`] files is a mass edit and is dropped.

use crate::sample::{Listing, MergedPr, SampleError};
use bohmian_code_review::RelativePath;
use std::collections::BTreeMap;

/// Non-mass-edit PRs in cold-build history.
pub const COLD_DEPTH: usize = 10;
/// Most changed files a history PR may have before it is a mass edit.
pub const MASS_EDIT_FILES: usize = 30;

/// A history PR's changed paths, or `None` for a mass edit.
pub type Changed = Option<Vec<RelativePath>>;

/// Classify PR `number`'s complete `listing` as history: a mass edit when
/// it lists more than [`MASS_EDIT_FILES`] files, otherwise its paths.
/// Completeness is checked first, so an incomplete listing never passes as
/// a mass edit.
///
/// # Errors
///
/// [`SampleError::IncompleteListing`] when the listing may be cut, then
/// [`SampleError::RejectedFile`] for a rejected entry.
pub fn changed(number: u64, listing: &Listing) -> Result<Changed, SampleError> {
    if listing.truncated {
        return Err(SampleError::IncompleteListing { number });
    }
    if let Some(rejected) = listing.rejected.first() {
        return Err(SampleError::RejectedFile {
            number,
            rejected: rejected.clone(),
        });
    }
    if listing.files.len() > MASS_EDIT_FILES {
        return Ok(None);
    }
    Ok(Some(
        listing.files.iter().map(|file| file.path.clone()).collect(),
    ))
}

/// One non-mass-edit history PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPr {
    /// PR number.
    pub number: u64,
    /// Merge time as `YYYY-MM-DDTHH:MM:SSZ`.
    pub merged_at: String,
    /// Every changed path in its listing.
    pub paths: Vec<RelativePath>,
}

/// File-page requests of history listings by the production step that
/// would need them. Each PR is one complete logical listing that may take
/// several page requests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListingReads {
    /// Page requests of the listings of PRs merged before the cold build
    /// point, mass edits included, until [`COLD_DEPTH`] non-mass-edit PRs
    /// were found.
    pub cold: usize,
    /// Page requests of each listing of a PR merged from the cold build
    /// point up to the newest sampled base, mass edits included, by PR
    /// number.
    pub incremental: BTreeMap<u64, usize>,
}

/// A repository's co-change history through its newest sampled base.
///
/// Invariant: `prs` holds, merge time then number descending, every
/// non-mass-edit PR merged from `cold_point` up to the newest sampled base
/// and exactly the [`COLD_DEPTH`] most recent merged before `cold_point`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Earliest sampled base commit time.
    pub cold_point: String,
    /// Non-mass-edit history PRs.
    prs: Vec<HistoryPr>,
    /// Listing page requests the history needed.
    pub reads: ListingReads,
}

impl History {
    /// Build history from `merged`, every merged PR in sample order, for
    /// sampled base times from `cold_point` through `newest_base`. `read`
    /// yields one PR's [`Changed`] with the file-page requests its listing
    /// took, and is called at most once per PR, only for PRs merged before
    /// `newest_base`.
    ///
    /// # Errors
    ///
    /// Returns `read`'s first error, and [`SampleError::ShortHistory`] when
    /// fewer than [`COLD_DEPTH`] non-mass-edit PRs merged before
    /// `cold_point`.
    pub async fn build<E: From<SampleError>>(
        merged: &[MergedPr],
        cold_point: &str,
        newest_base: &str,
        mut read: impl AsyncFnMut(u64) -> Result<(Changed, usize), E>,
    ) -> Result<History, E> {
        let mut history = History {
            cold_point: cold_point.to_owned(),
            prs: Vec::new(),
            reads: ListingReads::default(),
        };
        let mut cold = 0;
        for pr in merged {
            if pr.merged_at.as_str() >= newest_base {
                continue;
            }
            let before_cold = pr.merged_at.as_str() < cold_point;
            if before_cold && cold == COLD_DEPTH {
                break;
            }
            let (changed, pages) = read(pr.number).await?;
            if before_cold {
                history.reads.cold += pages;
            } else {
                history.reads.incremental.insert(pr.number, pages);
            }
            let Some(paths) = changed else {
                continue;
            };
            cold += usize::from(before_cold);
            history.prs.push(HistoryPr {
                number: pr.number,
                merged_at: pr.merged_at.clone(),
                paths,
            });
        }
        if cold < COLD_DEPTH {
            return Err(SampleError::ShortHistory { found: cold }.into());
        }
        Ok(history)
    }

    /// The history trial PR `number` sees at base time `base_time`: every
    /// history PR merged before it, never the trial PR itself.
    pub fn for_trial(&self, number: u64, base_time: &str) -> impl Iterator<Item = &HistoryPr> {
        self.prs
            .iter()
            .filter(move |pr| pr.number != number && pr.merged_at.as_str() < base_time)
    }
}

/// History contract checks on in-memory merged PRs and a counting fake
/// listing reader.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review_pipeline::{
        FileStatus, ObjectSha, PullRequestFile, PullRequestFilesPage, RejectedPath,
    };
    use std::cell::RefCell;

    /// A PR merged on day `day` of January 2026 at noon.
    fn merged(number: u64, day: u32) -> MergedPr {
        let sha = |c: &str| ObjectSha::new(&c.repeat(40)).expect("fixture sha is valid");
        MergedPr {
            number,
            merged_at: format!("2026-01-{day:02}T12:00:00Z"),
            base: sha("a"),
            head: sha("b"),
        }
    }

    /// A listing of `count` changed files.
    fn listing(count: usize) -> Listing {
        let files = (0..count)
            .map(|i| PullRequestFile {
                path: RelativePath::new(&format!("src/f{i}.py")).expect("canonical"),
                status: FileStatus::Modified,
                sha: ObjectSha::new(&"c".repeat(40)).expect("fixture sha is valid"),
                patch: None,
                previous_path: None,
            })
            .collect();
        let mut listing = Listing::default();
        let page = PullRequestFilesPage {
            files,
            rejected: Vec::new(),
            has_next: false,
        };
        listing.push(page, true);
        listing
    }

    /// Build history over PRs 1..=`last`, one per day, where PRs in `mass`
    /// are mass edits and PR 17's listing takes two pages; returns it with
    /// the PR numbers read, in order.
    async fn build(
        last: u32,
        mass: &[u64],
        cold_point: &str,
        newest_base: &str,
    ) -> (Result<History, SampleError>, Vec<u64>) {
        let mut prs: Vec<_> = (1..=last).map(|day| merged(u64::from(day), day)).collect();
        prs.reverse();
        let calls = RefCell::new(Vec::new());
        let history = History::build(&prs, cold_point, newest_base, async |number| {
            calls.borrow_mut().push(number);
            let files = if mass.contains(&number) { 31 } else { 30 };
            Ok((
                changed(number, &listing(files))?,
                if number == 17 { 2 } else { 1 },
            ))
        })
        .await;
        (history, calls.into_inner())
    }

    /// Numbers of the history PRs `history` shows trial `number` at `base`.
    fn seen(history: &History, number: u64, base: &str) -> Vec<u64> {
        history
            .for_trial(number, base)
            .map(|pr| pr.number)
            .collect()
    }

    /// Cold history is the ten most recent non-mass PRs before the cold
    /// point; later PRs join a trial's history only when merged strictly
    /// before its base time; mass edits never appear; each needed listing
    /// is read once, its page requests counted, and nothing after the
    /// newest base is read.
    #[tokio::test]
    async fn history_is_cold_depth_plus_time_eligible_increments() {
        let (history, calls) =
            build(25, &[8, 18], "2026-01-16T00:00:00Z", "2026-01-20T12:00:00Z").await;
        let history = history.expect("enough cold history");
        assert_eq!(calls, (5..=19).rev().collect::<Vec<u64>>());
        assert_eq!(
            history.reads,
            ListingReads {
                cold: 11,
                incremental: BTreeMap::from([(16, 1), (17, 2), (18, 1), (19, 1)]),
            }
        );
        let cold = [15, 14, 13, 12, 11, 10, 9, 7, 6, 5];
        assert_eq!(seen(&history, 99, "2026-01-16T00:00:00Z"), cold);
        assert_eq!(
            seen(&history, 99, "2026-01-17T12:00:00Z"),
            [&[16][..], &cold].concat()
        );
        assert_eq!(
            seen(&history, 99, "2026-01-17T12:00:01Z"),
            [&[17, 16][..], &cold].concat()
        );
        assert_eq!(
            seen(&history, 99, "2026-01-20T12:00:00Z"),
            [&[19, 17, 16][..], &cold].concat()
        );
    }

    /// A trial never sees its own PR, even when provider times would
    /// otherwise admit it.
    #[tokio::test]
    async fn history_excludes_trial_pr() {
        let (history, _) = build(20, &[], "2026-01-11T00:00:00Z", "2026-01-20T00:00:00Z").await;
        let history = history.expect("enough cold history");
        assert!(
            seen(&history, 12, "2026-01-19T00:00:00Z")
                .iter()
                .all(|&n| n != 12)
        );
        assert!(seen(&history, 11, "2026-01-19T00:00:00Z").contains(&12));
    }

    /// Fewer than ten non-mass PRs before the cold point, an incomplete
    /// listing, or a rejected entry fail the whole build, even above the
    /// mass-edit threshold; only a complete listing of 31 files is dropped.
    #[tokio::test]
    async fn history_fails_on_short_or_incomplete_input() {
        let (short, _) = build(12, &[1, 2], "2026-01-11T00:00:00Z", "2026-01-12T00:00:00Z").await;
        assert_eq!(short, Err(SampleError::ShortHistory { found: 8 }));

        let mut truncated = listing(3);
        truncated.truncated = true;
        assert_eq!(
            changed(7, &truncated),
            Err(SampleError::IncompleteListing { number: 7 })
        );
        let mut rejected = listing(3);
        let entry = RejectedPath {
            path: "../x.py".to_owned(),
            reason: "not canonical".to_owned(),
        };
        rejected.rejected.push(entry.clone());
        assert_eq!(
            changed(7, &rejected),
            Err(SampleError::RejectedFile {
                number: 7,
                rejected: entry
            })
        );
        let mut mass = listing(31);
        mass.rejected.push(RejectedPath {
            path: "../y.py".to_owned(),
            reason: "not canonical".to_owned(),
        });
        assert!(matches!(
            changed(7, &mass),
            Err(SampleError::RejectedFile { number: 7, .. })
        ));
        let mut ceiling = Listing::default();
        let page = PullRequestFilesPage {
            files: listing(31).files,
            rejected: Vec::new(),
            has_next: true,
        };
        assert!(!ceiling.push(page, true));
        assert_eq!(
            changed(7, &ceiling),
            Err(SampleError::IncompleteListing { number: 7 })
        );
        assert_eq!(changed(7, &listing(31)), Ok(None));
        assert_eq!(
            changed(7, &listing(30)).map(|c| c.map(|p| p.len())),
            Ok(Some(30))
        );

        let prs: Vec<_> = (1..=12)
            .rev()
            .map(|day| merged(u64::from(day), day))
            .collect();
        let failed = History::build(
            &prs,
            "2026-01-11T00:00:00Z",
            "2026-01-12T00:00:00Z",
            async |number| {
                if number == 9 {
                    Err(SampleError::IncompleteListing { number })
                } else {
                    Ok((Some(Vec::new()), 1))
                }
            },
        )
        .await;
        assert_eq!(failed, Err(SampleError::IncompleteListing { number: 9 }));
    }
}
