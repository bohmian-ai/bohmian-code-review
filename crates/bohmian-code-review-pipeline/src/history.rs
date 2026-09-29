//! Time-eligible co-change history read from merged pull requests.
//!
//! [`CoChangeHistory`] is the history a snapshot persists: every
//! non-mass-edit PR merged into the default branch over a contiguous span
//! ending strictly before the snapshot commit's committer time. A cold build
//! reads the [`COLD_DEPTH`] most recent; an update extends the prior
//! snapshot's history with every PR merged since, one listing each, keeping
//! at most [`MAX_HISTORY_PRS`]. A review ranks with the entries merged
//! before its base commit, never its own.
//!
//! [`History`] is the experiment's form of the same rule:
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
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Non-mass-edit PRs in cold-build history.
pub const COLD_DEPTH: usize = 10;
/// Most non-mass-edit PRs a snapshot's history keeps; an update drops the
/// oldest beyond it.
pub const MAX_HISTORY_PRS: usize = 1_000;
/// Closed-PR listing pages read per history read: 1,000 PRs at 100 per
/// page. A read that reaches it with more pages advertised fails: a cold
/// read stops before it once [`COLD_DEPTH`] PRs are proved, and an update
/// must reach its prior history's time or the listing's end.
pub const MAX_DISCOVERY_PAGES: u32 = 10;
/// Most changed files a history PR may have before it is a mass edit.
pub const MASS_EDIT_FILES: usize = 30;
/// File-listing pages read at most: 3,000 files at 100 per page.
pub const MAX_LISTING_PAGES: u32 = 30;
/// Most entries GitHub's PR-files endpoint returns; a listing this long
/// cannot prove it ended.
const MAX_LISTED_FILES: usize = 3_000;

/// Why history cannot be trusted; the caller must not rank with it.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
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
    /// A closed-PR listing page changed while later pages were read, so a
    /// page may have skipped a merged PR; a retry may succeed.
    #[error("closed pull requests changed while they were listed")]
    ListingMoved,
    /// The closed-PR listing reached [`MAX_DISCOVERY_PAGES`] with more
    /// pages advertised before a cold read proved [`COLD_DEPTH`]
    /// non-mass-edit PRs or an update reached its prior history's time,
    /// however many PRs an update proved.
    #[error(
        "closed-PR listing ended at its page bound after proving only {found} non-mass-edit PRs"
    )]
    UnprovedHistory {
        /// Non-mass-edit PRs proved before the bound.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// The co-change history a snapshot persists: source-free, only PR
/// numbers, merge times, and changed paths.
///
/// Invariant: `prs` holds, merge time then number descending, every
/// non-mass-edit PR merged into the default branch from the oldest entry's
/// merge time up to, but excluding, `before`; the listing may have proved
/// fewer than [`COLD_DEPTH`] exist. Empty history is valid.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoChangeHistory {
    /// Committer time of the snapshot commit.
    pub before: String,
    /// Non-mass-edit history PRs.
    pub prs: Vec<HistoryPr>,
}

impl CoChangeHistory {
    /// Read the history before commit time `before` on `branch`, extending
    /// `prior` when its span ends no later than `before`.
    ///
    /// Closed PRs are paged most recently updated first, at most
    /// [`MAX_DISCOVERY_PAGES`] pages. A PR merges no later than its last
    /// update, so after each page every PR merged after that page's oldest
    /// update is known; only those are read, newest merge first, each
    /// with one complete listing. Without `prior` reading stops at
    /// [`COLD_DEPTH`] non-mass-edit PRs; with it, at the first PR merged
    /// before `prior.before`, and `prior`'s entries follow. When the pages
    /// run out, history is what they proved and `prior` follows; reaching
    /// the page bound with more pages advertised fails, so an update never
    /// drops `prior` or skips a merge and the span stays contiguous. At most [`MAX_HISTORY_PRS`] are kept. Every page read
    /// before the last is read again afterwards and must list the same
    /// entries, so a listing that shifted mid-read fails.
    ///
    /// # Errors
    ///
    /// Provider errors, the errors of [`Listing::history_paths`],
    /// [`HistoryError::UnprovedHistory`], and [`HistoryError::ListingMoved`],
    /// a retry state.
    pub(crate) async fn read<E: From<GitHubError> + From<HistoryError>>(
        client: &GitHubClient,
        repository: &RepositoryIdentity,
        branch: &str,
        before: &str,
        prior: Option<&CoChangeHistory>,
    ) -> Result<CoChangeHistory, E> {
        let prior = prior.filter(|prior| prior.before.as_str() <= before);
        let since = prior.map(|prior| prior.before.as_str());
        let mut history = CoChangeHistory {
            before: before.to_owned(),
            prs: Vec::new(),
        };
        let (mut known, mut seen) = (Vec::<MergedPr>::new(), BTreeSet::new());
        let (mut pages, mut horizon, mut more) = (Vec::new(), None::<String>, true);
        loop {
            if since.is_none() && history.prs.len() == COLD_DEPTH {
                break;
            }
            let proven = known.first().filter(|pr| {
                horizon
                    .as_deref()
                    .is_none_or(|horizon| pr.merged_at.as_str() > horizon)
            });
            if let Some(pr) = proven {
                if since.is_some_and(|since| pr.merged_at.as_str() < since) {
                    break;
                }
                let pr = known.remove(0);
                let (listing, _) =
                    Listing::read(client, repository, pr.number, MAX_LISTING_PAGES).await?;
                if let Some(paths) = listing.history_paths(pr.number)? {
                    history.prs.push(HistoryPr {
                        number: pr.number,
                        merged_at: pr.merged_at,
                        paths,
                    });
                }
                continue;
            }
            let below_since = since
                .zip(horizon.as_deref())
                .is_some_and(|(since, horizon)| horizon < since);
            if !more || below_since {
                break;
            }
            if pages.len() == MAX_DISCOVERY_PAGES as usize {
                return Err(HistoryError::UnprovedHistory {
                    found: history.prs.len(),
                }
                .into());
            }
            let page = client
                .closed_pull_requests(repository, branch, pages.len() as u32 + 1)
                .await?;
            pages.push(page.listed);
            known.extend(
                page.merged
                    .into_iter()
                    .filter(|pr| pr.merged_at.as_str() < before && seen.insert(pr.number)),
            );
            known.sort_by(|a, b| (&b.merged_at, b.number).cmp(&(&a.merged_at, a.number)));
            more = page.has_next;
            horizon = match more {
                true => page.oldest_update.or(horizon),
                false => None,
            };
        }
        for (index, listed) in pages.iter().enumerate().take(pages.len().saturating_sub(1)) {
            let again = client
                .closed_pull_requests(repository, branch, index as u32 + 1)
                .await?;
            if again.listed != *listed {
                return Err(HistoryError::ListingMoved.into());
            }
        }
        if let Some(prior) = prior {
            history.prs.extend(prior.prs.iter().cloned());
        }
        history.prs.truncate(MAX_HISTORY_PRS);
        Ok(history)
    }

    /// Changed paths of every history PR a review of PR `number` whose base
    /// commit was committed at `base_time` may see: merged strictly before
    /// `base_time`, never PR `number` itself.
    pub fn for_review(
        &self,
        number: u64,
        base_time: &str,
    ) -> impl Iterator<Item = &[RelativePath]> {
        self.prs
            .iter()
            .filter(move |pr| pr.number != number && pr.merged_at.as_str() < base_time)
            .map(|pr| pr.paths.as_slice())
    }
}

/// Whether `text` is a `YYYY-MM-DDTHH:MM:SSZ` timestamp naming a real
/// Gregorian UTC instant; this fixed-width form is the one whose text
/// order is time order.
pub fn is_timestamp(text: &str) -> bool {
    const SHAPE: &[u8; 20] = b"dddd-dd-ddTdd:dd:ddZ";
    let shaped = text.len() == SHAPE.len()
        && text.bytes().zip(SHAPE).all(|(byte, &shape)| match shape {
            b'd' => byte.is_ascii_digit(),
            other => byte == other,
        });
    if !shaped {
        return false;
    }
    let field = |range: std::ops::Range<usize>| {
        text[range]
            .bytes()
            .fold(0_u32, |value, digit| value * 10 + u32::from(digit - b'0'))
    };
    let (year, month, day) = (field(0..4), field(5..7), field(8..10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day) && field(11..13) < 24 && field(14..16) < 60 && field(17..19) < 60
}

/// History contract checks on in-memory merged PRs, a counting fake
/// listing reader, and folded provider pages.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{FileStatus, Operation, RequestLimits};
    use crate::identity::{GitHubHost, InstallationToken};
    use crate::testing::RoutedTransport;
    use std::cell::RefCell;

    /// Closed-PR listing page `page` of `main`.
    fn closed(page: u32) -> String {
        format!(
            "/pulls?state=closed&base=main&sort=updated&direction=desc&per_page=100&page={page}"
        )
    }

    /// One listed closed PR updated at `updated`, merged then when `merged`.
    fn listed(number: u64, updated: &str, merged: bool) -> String {
        let merged_at = if merged {
            format!("\"{updated}\"")
        } else {
            "null".to_owned()
        };
        format!(
            r#"{{"number":{number},"updated_at":"{updated}","merged_at":{merged_at},"base":{{"ref":"main","sha":"{a}","repo":{{"id":1}}}},"head":{{"ref":"f","sha":"{b}"}}}}"#,
            a = "a".repeat(40),
            b = "b".repeat(40)
        )
    }

    /// A page response of `entries`, advertising a next page when `next`.
    fn page(entries: &[String], next: bool) -> (u16, Vec<(String, String)>, Vec<u8>) {
        let headers = match next {
            true => vec![("link".to_owned(), "<x>; rel=\"next\"".to_owned())],
            false => Vec::new(),
        };
        (
            200,
            headers,
            format!("[{}]", entries.join(",")).into_bytes(),
        )
    }

    /// Route PR `number`'s one-page listing of `count` files.
    fn route_listing(transport: &RoutedTransport, number: u64, count: usize) {
        let files: Vec<String> = (0..count)
            .map(|i| {
                format!(
                    r#"{{"filename":"src/p{number}/f{i}.rs","status":"modified","sha":"{}","patch":"@@ -1 +1 @@\n-a\n+b"}}"#,
                    "c".repeat(40)
                )
            })
            .collect();
        transport.json(
            &format!("/pulls/{number}/files?per_page=100&page=1"),
            &format!("[{}]", files.join(",")),
        );
    }

    /// PRs 1..=14 merged at noon on their day of January 2026, PR 13 a
    /// mass edit; PR 30 merged after the Jan 20 build point; PR 31 closed
    /// unmerged. Listed most recently updated first, split after six
    /// entries when `split`; page 1 answers `first_again` on a re-read.
    fn repository(split: bool, first_again: Option<String>) -> GitHubClient {
        let transport = RoutedTransport::default();
        let at = |day: u64| format!("2026-01-{day:02}T12:00:00Z");
        let mut entries = vec![listed(31, &at(25), false), listed(30, &at(21), true)];
        entries.extend((1..=14).rev().map(|n| listed(n, &at(n), true)));
        let (one, two) = entries.split_at(if split { 6 } else { entries.len() });
        let mut first = vec![page(one, split)];
        if let Some(again) = first_again {
            first.push(page(&[again], false));
        }
        transport.respond(&closed(1), first);
        transport.respond(&closed(2), vec![page(two, false)]);
        for number in (1..=14).chain([30]) {
            route_listing(&transport, number, if number == 13 { 31 } else { 1 });
        }
        GitHubClient::with_transport(
            GitHubHost::public(),
            InstallationToken::new("ghs_token").expect("token"),
            RequestLimits::default(),
            Box::new(transport),
        )
    }

    /// Read history before `before` over `client`, extending `prior`.
    async fn read(
        client: &GitHubClient,
        before: &str,
        prior: Option<&CoChangeHistory>,
    ) -> Result<CoChangeHistory, HistoryError> {
        let repository = RepositoryIdentity::new(1, "o", "r").expect("repo");
        CoChangeHistory::read::<GitHubOrHistory>(client, &repository, "main", before, prior)
            .await
            .map_err(|error| match error {
                GitHubOrHistory::History(error) => error,
                GitHubOrHistory::GitHub(error) => panic!("provider: {error}"),
            })
    }

    /// Either failure of a history read.
    #[derive(Debug)]
    enum GitHubOrHistory {
        /// A provider failure.
        GitHub(GitHubError),
        /// A history failure.
        History(HistoryError),
    }

    impl From<GitHubError> for GitHubOrHistory {
        /// Wraps a provider failure.
        fn from(error: GitHubError) -> GitHubOrHistory {
            GitHubOrHistory::GitHub(error)
        }
    }

    impl From<HistoryError> for GitHubOrHistory {
        /// Wraps a history failure.
        fn from(error: HistoryError) -> GitHubOrHistory {
            GitHubOrHistory::History(error)
        }
    }

    /// Numbers of `history`'s PRs.
    fn numbers(history: &CoChangeHistory) -> Vec<u64> {
        history.prs.iter().map(|pr| pr.number).collect()
    }

    /// A cold read keeps the ten most recent non-mass PRs merged before the
    /// build point, reading only PRs the pages proved, each listing once,
    /// the mass edit included; a two-page read re-checks page 1.
    #[tokio::test]
    async fn cold_history_reads_ten_proven_prs_before_the_commit() {
        let client = repository(true, None);
        let history = read(&client, "2026-01-20T00:00:00Z", None)
            .await
            .expect("history");
        assert_eq!(history.before, "2026-01-20T00:00:00Z");
        assert_eq!(numbers(&history), [14, 12, 11, 10, 9, 8, 7, 6, 5, 4]);
        assert_eq!(
            history.prs[0].paths,
            [RelativePath::new("src/p14/f0.rs").expect("path")]
        );
        let calls = client.calls();
        assert_eq!(calls.get(Operation::PullRequests), 3);
        assert_eq!(calls.get(Operation::PullRequestFiles), 11);
    }

    /// A closed PR updated while pages are read shifts the listing, so the
    /// re-read of page 1 fails the read as a retry state.
    #[tokio::test]
    async fn listing_changed_mid_read_fails() {
        let moved = listed(99, "2026-01-26T00:00:00Z", false);
        let client = repository(true, Some(moved));
        assert_eq!(
            read(&client, "2026-01-20T00:00:00Z", None).await,
            Err(HistoryError::ListingMoved)
        );
    }

    /// An update reads one listing per PR merged since the prior history's
    /// time, mass edits included, and keeps the prior entries after them;
    /// a prior history from a later time is ignored.
    #[tokio::test]
    async fn update_extends_prior_history_with_one_listing_per_merge() {
        let client = repository(false, None);
        let entry = |number: u64| HistoryPr {
            number,
            merged_at: format!("2026-01-{number:02}T12:00:00Z"),
            paths: Vec::new(),
        };
        let prior = CoChangeHistory {
            before: "2026-01-10T00:00:00Z".to_owned(),
            prs: vec![entry(9), entry(8)],
        };
        let history = read(&client, "2026-01-20T00:00:00Z", Some(&prior))
            .await
            .expect("history");
        assert_eq!(numbers(&history), [14, 12, 11, 10, 9, 8]);
        let calls = client.calls();
        assert_eq!(calls.get(Operation::PullRequests), 1);
        assert_eq!(calls.get(Operation::PullRequestFiles), 5);

        let later = CoChangeHistory {
            before: "2026-01-30T00:00:00Z".to_owned(),
            ..prior
        };
        let cold = read(
            &repository(false, None),
            "2026-01-20T00:00:00Z",
            Some(&later),
        )
        .await
        .expect("history");
        assert_eq!(numbers(&cold), [14, 12, 11, 10, 9, 8, 7, 6, 5, 4]);
    }

    /// No merged PRs is valid empty history; pages still advertised at the
    /// page bound that never prove cold depth or reach the prior time fail
    /// instead of returning a partial span, with or without a prior.
    #[tokio::test]
    async fn empty_history_is_valid_and_an_unproved_bound_fails() {
        let transport = RoutedTransport::default();
        transport.respond(&closed(1), vec![page(&[], false)]);
        let client = GitHubClient::with_transport(
            GitHubHost::public(),
            InstallationToken::new("ghs_token").expect("token"),
            RequestLimits::default(),
            Box::new(transport),
        );
        let empty = read(&client, "2026-01-20T00:00:00Z", None)
            .await
            .expect("history");
        assert!(empty.prs.is_empty());

        let bounded = || {
            let transport = RoutedTransport::default();
            for n in 1..=MAX_DISCOVERY_PAGES {
                let updated = format!("2026-01-{:02}T00:00:00Z", 30 - n);
                transport.respond(
                    &closed(n),
                    vec![page(&[listed(u64::from(n), &updated, false)], true)],
                );
            }
            GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                RequestLimits::default(),
                Box::new(transport),
            )
        };
        let prior = CoChangeHistory {
            before: "2026-01-02T00:00:00Z".to_owned(),
            prs: vec![HistoryPr {
                number: 1000,
                merged_at: "2026-01-01T00:00:00Z".to_owned(),
                paths: Vec::new(),
            }],
        };
        for prior in [Some(&prior), None] {
            let client = bounded();
            assert_eq!(
                read(&client, "2026-01-31T00:00:00Z", prior).await,
                Err(HistoryError::UnprovedHistory { found: 0 })
            );
            assert_eq!(
                client.calls().get(Operation::PullRequests),
                u64::from(MAX_DISCOVERY_PAGES)
            );
        }
    }

    /// An update whose ten pages each prove one new merged PR, with an
    /// eleventh page still advertised and the prior time not reached,
    /// fails rather than dropping the prior history or the unseen merge;
    /// a cold read of the same pages still completes at cold depth.
    #[tokio::test]
    async fn update_with_ten_new_prs_at_the_page_bound_fails() {
        let bounded = || {
            let transport = RoutedTransport::default();
            for n in 1..=MAX_DISCOVERY_PAGES {
                let merged = format!("2026-01-{:02}T12:00:00Z", 30 - n);
                let closed_unmerged = format!("2026-01-{:02}T00:00:00Z", 30 - n);
                transport.respond(
                    &closed(n),
                    vec![page(
                        &[
                            listed(u64::from(n), &merged, true),
                            listed(100 + u64::from(n), &closed_unmerged, false),
                        ],
                        true,
                    )],
                );
                route_listing(&transport, u64::from(n), 1);
            }
            GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                RequestLimits::default(),
                Box::new(transport),
            )
        };
        let prior = CoChangeHistory {
            before: "2026-01-02T00:00:00Z".to_owned(),
            prs: vec![HistoryPr {
                number: 1000,
                merged_at: "2026-01-01T00:00:00Z".to_owned(),
                paths: Vec::new(),
            }],
        };
        let client = bounded();
        assert_eq!(
            read(&client, "2026-01-31T00:00:00Z", Some(&prior)).await,
            Err(HistoryError::UnprovedHistory { found: 10 })
        );
        assert_eq!(
            client.calls().get(Operation::PullRequests),
            u64::from(MAX_DISCOVERY_PAGES)
        );

        let cold = read(&bounded(), "2026-01-31T00:00:00Z", None)
            .await
            .expect("cold history");
        assert_eq!(numbers(&cold), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    /// A page-2 entry that moves into page 1 below its unchanged first
    /// entry is caught by the full page-1 re-read, so the skipped merged PR
    /// fails the read as a retry state instead of leaving a gap.
    #[tokio::test]
    async fn entry_moving_below_the_first_fails() {
        let transport = RoutedTransport::default();
        let at = |day: u64| format!("2026-01-{day:02}T12:00:00Z");
        transport.respond(
            &closed(1),
            vec![
                page(&[listed(5, &at(5), true), listed(4, &at(4), true)], true),
                page(
                    &[
                        listed(5, &at(5), true),
                        listed(3, &at(3), true),
                        listed(4, &at(4), true),
                    ],
                    true,
                ),
            ],
        );
        transport.respond(&closed(2), vec![page(&[listed(2, &at(2), true)], false)]);
        for number in 2..=5 {
            route_listing(&transport, number, 1);
        }
        let client = GitHubClient::with_transport(
            GitHubHost::public(),
            InstallationToken::new("ghs_token").expect("token"),
            RequestLimits::default(),
            Box::new(transport),
        );
        assert_eq!(
            read(&client, "2026-01-20T00:00:00Z", None).await,
            Err(HistoryError::ListingMoved)
        );
    }

    /// A review sees only history merged strictly before its base commit's
    /// time and never its own PR.
    #[test]
    fn review_history_is_time_eligible_and_excludes_its_own_pr() {
        let entry = |number: u64, merged_at: &str| HistoryPr {
            number,
            merged_at: merged_at.to_owned(),
            paths: vec![RelativePath::new(&format!("src/{number}.rs")).expect("path")],
        };
        let history = CoChangeHistory {
            before: "2026-01-30T00:00:00Z".to_owned(),
            prs: vec![
                entry(4, "2026-01-20T00:00:00Z"),
                entry(3, "2026-01-10T00:00:00Z"),
                entry(2, "2026-01-05T00:00:00Z"),
            ],
        };
        let seen: Vec<&str> = history
            .for_review(3, "2026-01-20T00:00:00Z")
            .map(|paths| paths[0].as_str())
            .collect();
        assert_eq!(seen, ["src/2.rs"]);
    }

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
