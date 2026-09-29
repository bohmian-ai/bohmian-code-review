//! Time-eligible co-change history read from merged pull requests.
//!
//! A PR is merged before a commit when its merge time is earlier than the
//! commit's committer time. The cold build point is the earliest time a
//! caller ranks at; cold history is the [`COLD_DEPTH`] most recent
//! non-mass-edit PRs merged before it, and every non-mass-edit PR merged
//! from it up to the newest time extends history incrementally.
//! [`History::build`] reads each needed PR's file listing once, through the
//! newest time only, and a caller filters that one history by its own time
//! without any provider read. A PR changing more than [`MASS_EDIT_FILES`]
//! files is a mass edit and is dropped. Every listing is read complete or
//! the build fails; a partial listing never becomes history.

use crate::github::{
    GitHubClient, GitHubError, PullRequestFile, PullRequestFilesPage, RejectedPath,
};
use crate::identity::{ObjectSha, RepositoryIdentity};
use bohmian_code_review::RelativePath;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Non-mass-edit PRs in cold-build history.
pub const COLD_DEPTH: usize = 10;
/// Most changed files a history PR may have before it is a mass edit.
pub const MASS_EDIT_FILES: usize = 30;
/// File-listing pages read at most: 3,000 files at 100 per page.
pub const MAX_LISTING_PAGES: u32 = 30;
/// Most entries GitHub's PR-files endpoint returns; a listing this long
/// cannot prove it ended.
const MAX_LISTED_FILES: usize = 3_000;

/// Why history cannot be trusted; the caller must not rank with it.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HistoryError {
    /// The provider rejected an entry of a PR's file listing.
    #[error("PR #{number} lists an unusable file `{}`: {}", .rejected.path, .rejected.reason)]
    RejectedFile {
        /// PR number.
        number: u64,
        /// The rejected entry.
        rejected: RejectedPath,
    },
    /// A PR's file listing did not provably end within the provider's
    /// page or file limit.
    #[error("PR #{number} file listing is incomplete")]
    IncompleteListing {
        /// PR number.
        number: u64,
    },
    /// Fewer than [`COLD_DEPTH`] non-mass-edit PRs merged before the cold
    /// build point.
    #[error("only {found} non-mass-edit PRs merged before the cold build point")]
    ShortHistory {
        /// Non-mass-edit PRs found.
        found: usize,
    },
}

/// A merged PR with validated identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedPr {
    /// PR number.
    pub number: u64,
    /// Merge time as `YYYY-MM-DDTHH:MM:SSZ`, ordered as text.
    pub merged_at: String,
    /// Base commit.
    pub base: ObjectSha,
    /// Head commit.
    pub head: ObjectSha,
}

/// One PR's complete file listing, as read once from the provider.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    /// Valid entries in provider order.
    pub files: Vec<PullRequestFile>,
    /// Entries the provider client rejected.
    pub rejected: Vec<RejectedPath>,
    /// Whether the listing may be cut: the last page read still
    /// advertised a next page, or the provider's file ceiling was reached.
    pub truncated: bool,
    /// Paths of `files`, so a repeated path is caught at the fold.
    paths: BTreeSet<RelativePath>,
}

impl Listing {
    /// Read PR `number`'s file listing once, following at most `max_pages`
    /// pages until [`Listing::push`] ends it; returns it with the page
    /// requests it took. Completeness is not checked here; see
    /// [`Listing::complete`].
    ///
    /// # Errors
    ///
    /// Returns provider errors.
    pub async fn read(
        client: &GitHubClient,
        repository: &RepositoryIdentity,
        number: u64,
        max_pages: u32,
    ) -> Result<(Listing, usize), GitHubError> {
        let mut listing = Listing::default();
        let mut pages = 0;
        for page in 1..=max_pages {
            let read = client.pull_request_files(repository, number, page).await?;
            pages += 1;
            if !listing.push(read, page == max_pages) {
                break;
            }
        }
        Ok((listing, pages))
    }

    /// Fold one provider `page` into the listing; `last` marks the final
    /// page the caller will read. Accepted and rejected entries both count
    /// toward the provider's 3,000-file ceiling. An entry whose path is
    /// already listed, within this page or an earlier one, is rejected
    /// rather than kept, so `files` holds distinct paths in provider order.
    ///
    /// Returns whether another page must be read.
    pub fn push(&mut self, page: PullRequestFilesPage, last: bool) -> bool {
        for file in page.files {
            if self.paths.insert(file.path.clone()) {
                self.files.push(file);
            } else {
                self.rejected.push(RejectedPath {
                    path: file.path.as_str().to_owned(),
                    reason: "path listed twice".to_owned(),
                });
            }
        }
        self.rejected.extend(page.rejected);
        self.truncated =
            self.files.len() + self.rejected.len() >= MAX_LISTED_FILES || (page.has_next && last);
        page.has_next && !self.truncated
    }

    /// Whether PR `number`'s listing is complete and fully accepted.
    ///
    /// # Errors
    ///
    /// [`HistoryError::IncompleteListing`] when the listing may be cut,
    /// then [`HistoryError::RejectedFile`] for the first rejected entry.
    pub fn complete(&self, number: u64) -> Result<(), HistoryError> {
        if self.truncated {
            return Err(HistoryError::IncompleteListing { number });
        }
        match self.rejected.first() {
            Some(rejected) => Err(HistoryError::RejectedFile {
                number,
                rejected: rejected.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Classify PR `number`'s listing as history: `None` for a mass edit of
    /// more than [`MASS_EDIT_FILES`] files, otherwise its paths.
    /// Completeness is checked first, so an incomplete listing never passes
    /// as a mass edit.
    ///
    /// # Errors
    ///
    /// The errors of [`Listing::complete`].
    pub fn history_paths(&self, number: u64) -> Result<Option<Vec<RelativePath>>, HistoryError> {
        self.complete(number)?;
        if self.files.len() > MASS_EDIT_FILES {
            return Ok(None);
        }
        Ok(Some(
            self.files.iter().map(|file| file.path.clone()).collect(),
        ))
    }
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

/// File-page requests of history listings by the step that needed them.
/// Each PR is one complete logical listing that may take several page
/// requests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListingReads {
    /// Page requests of the listings of PRs merged before the cold build
    /// point, mass edits included, until [`COLD_DEPTH`] non-mass-edit PRs
    /// were found.
    pub cold: usize,
    /// Page requests of each listing of a PR merged from the cold build
    /// point up to the newest time, mass edits included, by PR number.
    pub incremental: BTreeMap<u64, usize>,
}

/// A repository's co-change history through its newest time.
///
/// Invariant: `prs` holds, merge time then number descending, every
/// non-mass-edit PR merged from `cold_point` up to the newest time and
/// exactly the [`COLD_DEPTH`] most recent merged before `cold_point`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Earliest time history is ranked at.
    pub cold_point: String,
    /// Non-mass-edit history PRs.
    prs: Vec<HistoryPr>,
    /// Listing page requests the history needed.
    pub reads: ListingReads,
}

impl History {
    /// Build history from `merged`, every merged PR merge time then number
    /// descending, for times from `cold_point` through `newest`. `read`
    /// yields one PR's [`Listing::history_paths`] with the file-page
    /// requests its listing took, and is called at most once per PR, only
    /// for PRs merged before `newest`.
    ///
    /// # Errors
    ///
    /// Returns `read`'s first error, and [`HistoryError::ShortHistory`] when
    /// fewer than [`COLD_DEPTH`] non-mass-edit PRs merged before
    /// `cold_point`.
    pub async fn build<E: From<HistoryError>>(
        merged: &[MergedPr],
        cold_point: &str,
        newest: &str,
        mut read: impl AsyncFnMut(u64) -> Result<(Option<Vec<RelativePath>>, usize), E>,
    ) -> Result<History, E> {
        let mut history = History {
            cold_point: cold_point.to_owned(),
            prs: Vec::new(),
            reads: ListingReads::default(),
        };
        let mut cold = 0;
        for pr in merged {
            if pr.merged_at.as_str() >= newest {
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
            return Err(HistoryError::ShortHistory { found: cold }.into());
        }
        Ok(history)
    }

    /// The history PR `number` sees at time `at`: every history PR merged
    /// before it, never PR `number` itself.
    pub fn for_trial(&self, number: u64, at: &str) -> impl Iterator<Item = &HistoryPr> {
        self.prs
            .iter()
            .filter(move |pr| pr.number != number && pr.merged_at.as_str() < at)
    }

    /// The history a review of PR `number` sees now: every history PR but
    /// `number` itself.
    pub fn for_review(&self, number: u64) -> impl Iterator<Item = &HistoryPr> {
        self.prs.iter().filter(move |pr| pr.number != number)
    }
}

/// History contract checks on in-memory merged PRs, a counting fake
/// listing reader, and folded provider pages.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::FileStatus;
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

    /// One modified listing entry at `path`.
    fn file(path: &str) -> PullRequestFile {
        PullRequestFile {
            path: RelativePath::new(path).expect("fixture path is canonical"),
            status: FileStatus::Modified,
            sha: ObjectSha::new(&"c".repeat(40)).expect("fixture sha is valid"),
            patch: Some("@@ -1 +1 @@\n-a\n+b".to_owned()),
            previous_path: None,
        }
    }

    /// A listing of `count` changed files.
    fn listing(count: usize) -> Listing {
        let mut listing = Listing::default();
        let page = PullRequestFilesPage {
            files: (0..count).map(|i| file(&format!("src/f{i}.py"))).collect(),
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
        newest: &str,
    ) -> (Result<History, HistoryError>, Vec<u64>) {
        let mut prs: Vec<_> = (1..=last).map(|day| merged(u64::from(day), day)).collect();
        prs.reverse();
        let calls = RefCell::new(Vec::new());
        let history = History::build(&prs, cold_point, newest, async |number| {
            calls.borrow_mut().push(number);
            let files = if mass.contains(&number) { 31 } else { 30 };
            Ok((
                listing(files).history_paths(number)?,
                if number == 17 { 2 } else { 1 },
            ))
        })
        .await;
        (history, calls.into_inner())
    }

    /// Numbers of the history PRs `history` shows PR `number` at `at`.
    fn seen(history: &History, number: u64, at: &str) -> Vec<u64> {
        history.for_trial(number, at).map(|pr| pr.number).collect()
    }

    /// Cold history is the ten most recent non-mass PRs before the cold
    /// point; later PRs join a trial's history only when merged strictly
    /// before its time; mass edits never appear; each needed listing is
    /// read once, its page requests counted, and nothing after the newest
    /// time is read.
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

    /// A trial or review never sees its own PR, even when provider times
    /// would otherwise admit it; a review sees every other history PR.
    #[tokio::test]
    async fn history_excludes_own_pr() {
        let (history, _) = build(20, &[], "2026-01-11T00:00:00Z", "2026-01-20T00:00:00Z").await;
        let history = history.expect("enough cold history");
        assert!(
            seen(&history, 12, "2026-01-19T00:00:00Z")
                .iter()
                .all(|&n| n != 12)
        );
        assert!(seen(&history, 11, "2026-01-19T00:00:00Z").contains(&12));
        let review: Vec<u64> = history.for_review(12).map(|pr| pr.number).collect();
        assert_eq!(
            review,
            [
                19, 18, 17, 16, 15, 14, 13, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1
            ]
        );
    }

    /// Fewer than ten non-mass PRs before the cold point, an incomplete
    /// listing, or a rejected entry fail the whole build, even above the
    /// mass-edit threshold; only a complete listing of 31 files is dropped.
    #[tokio::test]
    async fn history_fails_on_short_or_incomplete_input() {
        let (short, _) = build(12, &[1, 2], "2026-01-11T00:00:00Z", "2026-01-12T00:00:00Z").await;
        assert_eq!(short, Err(HistoryError::ShortHistory { found: 8 }));

        let mut truncated = listing(3);
        truncated.truncated = true;
        assert_eq!(
            truncated.history_paths(7),
            Err(HistoryError::IncompleteListing { number: 7 })
        );
        let mut rejected = listing(3);
        let entry = RejectedPath {
            path: "../x.py".to_owned(),
            reason: "not canonical".to_owned(),
        };
        rejected.rejected.push(entry.clone());
        assert_eq!(
            rejected.history_paths(7),
            Err(HistoryError::RejectedFile {
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
            mass.history_paths(7),
            Err(HistoryError::RejectedFile { number: 7, .. })
        ));
        let mut ceiling = Listing::default();
        let page = PullRequestFilesPage {
            files: listing(31).files,
            rejected: Vec::new(),
            has_next: true,
        };
        assert!(!ceiling.push(page, true));
        assert_eq!(
            ceiling.history_paths(7),
            Err(HistoryError::IncompleteListing { number: 7 })
        );
        assert_eq!(listing(31).history_paths(7), Ok(None));
        assert_eq!(
            listing(30).history_paths(7).map(|c| c.map(|p| p.len())),
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
                    Err(HistoryError::IncompleteListing { number })
                } else {
                    Ok((Some(Vec::new()), 1))
                }
            },
        )
        .await;
        assert_eq!(failed, Err(HistoryError::IncompleteListing { number: 9 }));
    }

    /// A listing folded from provider pages is incomplete when a page past
    /// the last is advertised or when accepted plus rejected entries reach
    /// the provider's file ceiling, even with no next page; a shorter
    /// terminal page is complete.
    #[test]
    fn listing_fails_at_provider_limits() {
        let page =
            |n: usize, accepted: usize, rejected: usize, has_next: bool| PullRequestFilesPage {
                files: (0..accepted)
                    .map(|i| file(&format!("docs/p{n}/f{i}.md")))
                    .collect(),
                rejected: (0..rejected)
                    .map(|i| RejectedPath {
                        path: format!("../r{i}"),
                        reason: "not canonical".to_owned(),
                    })
                    .collect(),
                has_next,
            };
        let fold = |pages: Vec<PullRequestFilesPage>| {
            let mut listing = Listing::default();
            let last = pages.len();
            for (read, page) in pages.into_iter().enumerate() {
                if !listing.push(page, read + 1 == last) {
                    break;
                }
            }
            listing.complete(3).err()
        };
        let capped = (1..=30).map(|n| page(n, 99, 1, n < 30)).collect();
        assert_eq!(
            fold(capped),
            Some(HistoryError::IncompleteListing { number: 3 })
        );
        let advertised = (1..=30).map(|n| page(n, 50, 0, true)).collect();
        assert_eq!(
            fold(advertised),
            Some(HistoryError::IncompleteListing { number: 3 })
        );
        assert_eq!(
            fold(vec![page(1, 100, 0, true), page(2, 40, 0, false)]),
            None
        );
    }

    /// A canonical path listed twice, within one page or across pages and
    /// whatever its status or patch, is a rejected entry; distinct paths
    /// are complete.
    #[test]
    fn listing_rejects_repeated_paths() {
        let fold = |pages: Vec<Vec<PullRequestFile>>| {
            let mut listing = Listing::default();
            let last = pages.len();
            for (read, files) in pages.into_iter().enumerate() {
                let page = PullRequestFilesPage {
                    files,
                    rejected: Vec::new(),
                    has_next: read + 1 < last,
                };
                if !listing.push(page, read + 1 == last) {
                    break;
                }
            }
            listing
        };
        let repeated = |listing: &Listing| match listing.complete(4) {
            Err(HistoryError::RejectedFile { number, rejected }) => {
                number == 4 && rejected.path == "src/a.py"
            }
            _ => false,
        };
        let first = file("src/a.py");
        let mut again = file("src/a.py");
        again.status = FileStatus::Added;
        again.patch = None;
        assert!(repeated(&fold(vec![vec![first.clone(), again.clone()]])));
        assert!(repeated(&fold(vec![vec![first.clone()], vec![again]])));
        let distinct = fold(vec![vec![first], vec![file("src/b.py")]]);
        assert_eq!(distinct.complete(4), Ok(()));
        assert_eq!(distinct.files.len(), 2);
    }
}
