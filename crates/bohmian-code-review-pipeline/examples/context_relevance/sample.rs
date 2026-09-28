//! PR sample selection for the context-relevance evaluation.
//!
//! Discovery reads closed PRs newest-updated first. A PR's merge time never
//! exceeds its update time, so once the listing has passed update time `t`,
//! every PR merged after `t` is known and can be settled in final order:
//! merge time descending, then PR number descending. Each settled PR is
//! qualified from its one file listing, path eligibility, and its base
//! snapshot's declarations. Missing or malformed patches are file-local
//! omissions; unusable identities, rejected entries, and incomplete
//! listings fail the run.

use bohmian_code_review::{Language, RelativePath, parse_hunks};
use bohmian_code_review_pipeline::{FileStatus, ObjectSha, PullRequestFile, RejectedPath};
use serde::Deserialize;
use std::collections::BTreeSet;
use thiserror::Error;

/// PRs sampled per repository.
pub const SAMPLE_SIZE: usize = 50;
/// Fewest changed Python or Rust files a sampled PR may have.
const MIN_SOURCE_FILES: usize = 2;
/// Most changed Python or Rust files a sampled PR may have.
const MAX_SOURCE_FILES: usize = 15;

/// Why sampling cannot produce a trustworthy sample; the run stops.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SampleError {
    /// A listed PR has an unusable number, commit, or timestamp.
    #[error("PR #{number} is unusable: {detail}")]
    UnusablePr {
        /// PR number as listed.
        number: u64,
        /// What was wrong.
        detail: String,
    },
    /// The PR listing named one PR twice, so pages shifted mid-read.
    #[error("PR #{number} listed twice; the PR listing changed while it was read")]
    DuplicatePr {
        /// Repeated PR number.
        number: u64,
    },
    /// The configured cutoff is not a `YYYY-MM-DDTHH:MM:SSZ` timestamp.
    #[error("cutoff `{0}` is not a YYYY-MM-DDTHH:MM:SSZ timestamp")]
    InvalidCutoff(String),
    /// The provider rejected an entry of a PR's file listing.
    #[error("PR #{number} lists an unusable file `{}`: {}", .rejected.path, .rejected.reason)]
    RejectedFile {
        /// PR number.
        number: u64,
        /// The rejected entry.
        rejected: RejectedPath,
    },
    /// A PR's file listing did not end within the provider's page limit.
    #[error("PR #{number} file listing is incomplete")]
    IncompleteListing {
        /// PR number.
        number: u64,
    },
    /// A PR's base snapshot does not cover its whole base tree.
    #[error("PR #{number} base snapshot at {base} is incomplete: {detail}")]
    IncompleteSnapshot {
        /// PR number.
        number: u64,
        /// Base commit.
        base: ObjectSha,
        /// Why the snapshot is incomplete.
        detail: String,
    },
    /// Fewer than [`SAMPLE_SIZE`] PRs qualified before the listing ended.
    #[error("only {found} of {SAMPLE_SIZE} qualifying PRs before the cutoff")]
    TooFew {
        /// Qualifying PRs found.
        found: usize,
    },
}

/// Wire shape of one `GET /repos/{owner}/{repo}/pulls` entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ListedPr {
    /// PR number.
    pub number: u64,
    /// Last update time.
    pub updated_at: String,
    /// Merge time, absent when closed unmerged.
    pub merged_at: Option<String>,
    /// Base side.
    pub base: ListedRef,
    /// Head side.
    pub head: ListedRef,
}

/// Wire shape of a listed PR's base or head.
#[derive(Debug, Clone, Deserialize)]
pub struct ListedRef {
    /// Branch name.
    #[serde(rename = "ref")]
    pub name: String,
    /// Commit id.
    pub sha: String,
}

/// A merged PR with validated identity, ready for qualification.
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

/// Merged PRs discovered so far, released in final sample order once no
/// unread PR can precede them.
///
/// Invariant: every PR in `pending` merged into `branch` strictly before
/// `cutoff`, and each PR number is offered at most once.
pub struct Discovery {
    branch: String,
    cutoff: String,
    seen: BTreeSet<u64>,
    pending: Vec<MergedPr>,
}

impl Discovery {
    /// Discovery of PRs merged into `branch` strictly before `cutoff`.
    ///
    /// # Errors
    ///
    /// [`SampleError::InvalidCutoff`] when `cutoff` is not a
    /// `YYYY-MM-DDTHH:MM:SSZ` timestamp.
    pub fn new(branch: &str, cutoff: &str) -> Result<Discovery, SampleError> {
        if !is_timestamp(cutoff) {
            return Err(SampleError::InvalidCutoff(cutoff.to_owned()));
        }
        Ok(Discovery {
            branch: branch.to_owned(),
            cutoff: cutoff.to_owned(),
            seen: BTreeSet::new(),
            pending: Vec::new(),
        })
    }

    /// Record one listed PR, keeping it only when it merged into the branch
    /// before the cutoff.
    ///
    /// # Errors
    ///
    /// [`SampleError::DuplicatePr`] for a number already offered and
    /// [`SampleError::UnusablePr`] for an unusable number, update or merge
    /// time, or base/head commit.
    pub fn offer(&mut self, listed: ListedPr) -> Result<(), SampleError> {
        let number = listed.number;
        let unusable = |detail: String| SampleError::UnusablePr { number, detail };
        if number == 0 {
            return Err(unusable("number 0".to_owned()));
        }
        if !self.seen.insert(number) {
            return Err(SampleError::DuplicatePr { number });
        }
        if !is_timestamp(&listed.updated_at) {
            return Err(unusable(format!("update time `{}`", listed.updated_at)));
        }
        let Some(merged_at) = listed.merged_at else {
            return Ok(());
        };
        if !is_timestamp(&merged_at) {
            return Err(unusable(format!("merge time `{merged_at}`")));
        }
        if listed.base.name != self.branch || merged_at >= self.cutoff {
            return Ok(());
        }
        let commit = |sha: &str| ObjectSha::new(sha).map_err(|error| unusable(error.to_string()));
        self.pending.push(MergedPr {
            number,
            base: commit(&listed.base.sha)?,
            head: commit(&listed.head.sha)?,
            merged_at,
        });
        Ok(())
    }

    /// Remove and return, in sample order, every pending PR merged strictly
    /// after `read_until`, the oldest update time read so far; `None` once
    /// the listing is exhausted releases everything.
    pub fn settled(&mut self, read_until: Option<&str>) -> Vec<MergedPr> {
        let (mut settled, pending): (Vec<_>, Vec<_>) = self
            .pending
            .drain(..)
            .partition(|pr| read_until.is_none_or(|until| pr.merged_at.as_str() > until));
        self.pending = pending;
        settled.sort_by(|a, b| (&b.merged_at, b.number).cmp(&(&a.merged_at, a.number)));
        settled
    }
}

/// Whether `text` is a `YYYY-MM-DDTHH:MM:SSZ` timestamp, the one format
/// whose text order is time order.
fn is_timestamp(text: &str) -> bool {
    const SHAPE: &[u8; 20] = b"dddd-dd-ddTdd:dd:ddZ";
    text.len() == SHAPE.len()
        && text.bytes().zip(SHAPE).all(|(byte, &shape)| match shape {
            b'd' => byte.is_ascii_digit(),
            other => byte == other,
        })
}

/// One PR's complete file listing, as read once from the provider.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    /// Valid entries in provider order.
    pub files: Vec<PullRequestFile>,
    /// Entries the provider client rejected.
    pub rejected: Vec<RejectedPath>,
    /// Whether the last page read still advertised a next page.
    pub truncated: bool,
}

/// A PR's changed Python or Rust files, screened from its listing without
/// snapshot facts.
///
/// Invariant: `source` holds exactly the listing's Python or Rust entries
/// in listing order, and `changed_lines` marks those whose patch parsed to
/// at least one head-side changed line.
#[derive(Debug, Clone)]
pub struct Screen {
    /// The PR's Python or Rust entries.
    pub source: Vec<PullRequestFile>,
    changed_lines: Vec<bool>,
    /// Source paths whose patch was missing or malformed.
    pub omitted: Vec<Omitted>,
}

/// A changed file whose patch supplies no changed lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Omitted {
    /// Head-side path.
    pub path: RelativePath,
    /// Why no changed lines were read.
    pub reason: OmittedReason,
}

/// Why a changed file's patch was unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmittedReason {
    /// The listing carried no patch.
    MissingPatch,
    /// The patch did not parse as hunks.
    MalformedPatch(String),
}

impl Screen {
    /// Screen `listing` for PR `number`, keeping entries `language` routes
    /// to Python or Rust.
    ///
    /// # Errors
    ///
    /// [`SampleError::RejectedFile`] for any rejected entry and
    /// [`SampleError::IncompleteListing`] for a truncated listing.
    pub fn new(
        number: u64,
        listing: Listing,
        language: impl Fn(&RelativePath) -> Option<Language>,
    ) -> Result<Screen, SampleError> {
        if let Some(rejected) = listing.rejected.into_iter().next() {
            return Err(SampleError::RejectedFile { number, rejected });
        }
        if listing.truncated {
            return Err(SampleError::IncompleteListing { number });
        }
        let mut screen = Screen {
            source: Vec::new(),
            changed_lines: Vec::new(),
            omitted: Vec::new(),
        };
        for file in listing.files {
            if !matches!(
                language(&file.path),
                Some(Language::Python | Language::Rust)
            ) {
                continue;
            }
            let changed = match file.patch.as_deref().map(parse_hunks) {
                Some(Ok(ranges)) => !ranges.is_empty(),
                parsed => {
                    let reason = match parsed {
                        Some(Err(error)) => OmittedReason::MalformedPatch(error.to_string()),
                        _ => OmittedReason::MissingPatch,
                    };
                    let path = file.path.clone();
                    screen.omitted.push(Omitted { path, reason });
                    false
                }
            };
            screen.changed_lines.push(changed);
            screen.source.push(file);
        }
        Ok(screen)
    }

    /// Whether the PR could qualify for some base snapshot: 2–15 source
    /// files, and a modified file plus another file with changed lines.
    pub fn plausible(&self) -> bool {
        self.qualifies(|_| true)
    }

    /// Whether the PR qualifies given `declared`, which answers whether the
    /// base snapshot holds declarations for a path.
    pub fn qualifies(&self, declared: impl Fn(&RelativePath) -> bool) -> bool {
        (MIN_SOURCE_FILES..=MAX_SOURCE_FILES).contains(&self.source.len())
            && self.source.iter().enumerate().any(|(withheld, file)| {
                file.status == FileStatus::Modified
                    && declared(&file.path)
                    && (0..self.source.len())
                        .any(|other| other != withheld && self.changed_lines[other])
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A listed PR merged into `main` at `merged_at` (or unmerged).
    fn listed(number: u64, updated_at: &str, merged_at: Option<&str>) -> ListedPr {
        let side = |sha: &str| ListedRef {
            name: "main".to_owned(),
            sha: sha.repeat(40),
        };
        ListedPr {
            number,
            updated_at: updated_at.to_owned(),
            merged_at: merged_at.map(str::to_owned),
            base: side("a"),
            head: side("b"),
        }
    }

    /// One modified listing entry with `patch`.
    fn file(path: &str, status: FileStatus, patch: Option<&str>) -> PullRequestFile {
        PullRequestFile {
            path: RelativePath::new(path).expect("fixture path is canonical"),
            status,
            sha: ObjectSha::new(&"c".repeat(40)).expect("fixture sha is valid"),
            patch: patch.map(str::to_owned),
            previous_path: None,
        }
    }

    /// Python for `.py`, Rust for `.rs`, anything else unsupported.
    fn language(path: &RelativePath) -> Option<Language> {
        match path.as_str().rsplit_once('.') {
            Some((_, "py")) => Some(Language::Python),
            Some((_, "rs")) => Some(Language::Rust),
            Some((_, "ts")) => Some(Language::Typescript),
            _ => None,
        }
    }

    const PATCH: &str = "@@ -1 +1 @@\n-a\n+b";

    /// Discovery keeps merged PRs before the cutoff on the base branch and
    /// releases only those no unread PR can precede, merge time then
    /// number descending.
    #[test]
    fn discovery_orders_and_settles_merged_prs() {
        let mut discovery = Discovery::new("main", "2026-01-10T00:00:00Z").expect("valid cutoff");
        for pr in [
            listed(1, "2026-01-12T00:00:00Z", Some("2026-01-11T00:00:00Z")),
            listed(2, "2026-01-09T00:00:00Z", None),
            listed(3, "2026-01-09T00:00:00Z", Some("2026-01-05T00:00:00Z")),
            listed(4, "2026-01-08T00:00:00Z", Some("2026-01-05T00:00:00Z")),
            listed(5, "2026-01-08T00:00:00Z", Some("2026-01-08T00:00:00Z")),
        ] {
            discovery.offer(pr).expect("usable PR");
        }
        let mut other = listed(6, "2026-01-07T00:00:00Z", Some("2026-01-07T00:00:00Z"));
        other.base.name = "release".to_owned();
        discovery.offer(other).expect("usable PR");
        assert!(discovery.settled(Some("2026-01-08T00:00:00Z")).is_empty());
        let numbers = |prs: Vec<MergedPr>| prs.iter().map(|pr| pr.number).collect::<Vec<_>>();
        assert_eq!(
            numbers(discovery.settled(Some("2026-01-07T00:00:00Z"))),
            [5]
        );
        assert_eq!(numbers(discovery.settled(None)), [4, 3]);
    }

    /// Unusable identities, duplicates, and a bad cutoff stop the run.
    #[test]
    fn discovery_fails_on_unusable_input() {
        assert_eq!(
            Discovery::new("main", "2026-01-10").err(),
            Some(SampleError::InvalidCutoff("2026-01-10".to_owned()))
        );
        let mut discovery = Discovery::new("main", "2026-01-10T00:00:00Z").expect("valid cutoff");
        let unusable = |result: Result<(), SampleError>| {
            matches!(result, Err(SampleError::UnusablePr { number: 7, .. }))
        };
        assert!(unusable(discovery.offer(listed(7, "2026-01-09", None))));
        let mut discovery = Discovery::new("main", "2026-01-10T00:00:00Z").expect("valid cutoff");
        let mut bad = listed(7, "2026-01-09T00:00:00Z", Some("2026-01-09T00:00:00Z"));
        bad.head.sha = "nope".to_owned();
        assert!(unusable(discovery.offer(bad)));
        let mut discovery = Discovery::new("main", "2026-01-10T00:00:00Z").expect("valid cutoff");
        discovery
            .offer(listed(8, "2026-01-09T00:00:00Z", None))
            .expect("usable PR");
        assert_eq!(
            discovery.offer(listed(8, "2026-01-09T00:00:00Z", None)),
            Err(SampleError::DuplicatePr { number: 8 })
        );
    }

    /// A PR qualifies only with 2–15 Python/Rust files, a modified file the
    /// base declares, and another file with changed lines; missing and
    /// malformed patches are omissions that supply no changed lines.
    #[test]
    fn screen_qualifies_and_records_omissions() {
        let declared = |path: &RelativePath| path.as_str() != "src/new.py";
        let screen = |files: Vec<PullRequestFile>| {
            Screen::new(
                1,
                Listing {
                    files,
                    ..Listing::default()
                },
                language,
            )
            .expect("complete listing")
        };
        let qualifying = screen(vec![
            file("src/a.py", FileStatus::Modified, Some(PATCH)),
            file("src/b.py", FileStatus::Added, Some(PATCH)),
            file("web/c.ts", FileStatus::Modified, Some(PATCH)),
        ]);
        assert_eq!(qualifying.source.len(), 2);
        assert!(qualifying.qualifies(declared));

        let omitted = screen(vec![
            file("src/a.rs", FileStatus::Modified, Some(PATCH)),
            file("src/b.rs", FileStatus::Added, None),
            file("src/c.rs", FileStatus::Added, Some("not a patch")),
        ]);
        assert!(!omitted.qualifies(declared));
        assert_eq!(
            omitted
                .omitted
                .iter()
                .map(|o| o.path.as_str())
                .collect::<Vec<_>>(),
            ["src/b.rs", "src/c.rs"]
        );
        assert_eq!(omitted.omitted[0].reason, OmittedReason::MissingPatch);
        assert!(matches!(
            omitted.omitted[1].reason,
            OmittedReason::MalformedPatch(_)
        ));

        let undeclared = screen(vec![
            file("src/new.py", FileStatus::Modified, Some(PATCH)),
            file("src/b.py", FileStatus::Added, Some(PATCH)),
        ]);
        assert!(undeclared.plausible());
        assert!(!undeclared.qualifies(declared));

        let renamed_only = screen(vec![
            file("src/a.py", FileStatus::Renamed, Some(PATCH)),
            file("src/b.py", FileStatus::Modified, Some(PATCH)),
        ]);
        assert!(renamed_only.qualifies(declared));
        assert!(!renamed_only.qualifies(|path| path.as_str() == "src/a.py"));

        assert!(!screen(vec![file("src/a.py", FileStatus::Modified, Some(PATCH))]).plausible());
        let many = (0..16)
            .map(|i| file(&format!("src/f{i}.rs"), FileStatus::Modified, Some(PATCH)))
            .collect();
        assert!(!screen(many).plausible());
    }

    /// A rejected entry or a truncated listing fails the run.
    #[test]
    fn screen_fails_on_rejected_or_incomplete_listing() {
        let rejected = RejectedPath {
            path: "../x.py".to_owned(),
            reason: "not canonical".to_owned(),
        };
        let listing = Listing {
            rejected: vec![rejected.clone()],
            ..Listing::default()
        };
        assert_eq!(
            Screen::new(3, listing, language).err(),
            Some(SampleError::RejectedFile {
                number: 3,
                rejected
            })
        );
        let listing = Listing {
            truncated: true,
            ..Listing::default()
        };
        assert_eq!(
            Screen::new(3, listing, language).err(),
            Some(SampleError::IncompleteListing { number: 3 })
        );
    }
}
