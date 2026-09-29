//! PR sample selection for the context-relevance evaluation.
//!
//! Discovery reads every PR oldest-created first before qualifying any, so
//! activity during the read can only append to the listing, never shift a
//! page already read. Merged PRs are then ordered by merge time descending,
//! then PR number descending. Each is qualified in that order from its one
//! file listing, its repository's source-language eligibility, and its base
//! snapshot's declarations. Missing or malformed patches are file-local
//! omissions; unusable identities, rejected entries, and incomplete
//! listings fail the run.

use bohmian_code_review::{Language, RelativePath, SymbolicContext, parse_hunks};
use bohmian_code_review_pipeline::{
    FileStatus, ObjectSha, PullRequestFile, PullRequestFilesPage, RejectedPath,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use thiserror::Error;

/// PRs sampled per repository.
pub const SAMPLE_SIZE: usize = 50;
/// Fewest changed source files a sampled PR may have.
const MIN_SOURCE_FILES: usize = 2;
/// Most changed source files a sampled PR may have.
const MAX_SOURCE_FILES: usize = 15;
/// Most entries GitHub's PR-files endpoint returns; a listing this long
/// cannot prove it ended.
const MAX_LISTED_FILES: usize = 3_000;

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
    /// A PR's file listing did not provably end within the provider's
    /// page or file limit.
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
    /// Fewer than [`crate::history::COLD_DEPTH`] non-mass-edit PRs merged
    /// before the cold build point.
    #[error("only {found} non-mass-edit PRs merged before the cold build point")]
    ShortHistory {
        /// Non-mass-edit PRs found.
        found: usize,
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

/// Merged PRs discovered so far, released in final sample order once the
/// listing has been read to its end.
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
    /// time, a merge time after the update time, or base/head commit.
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
        if merged_at > listed.updated_at {
            return Err(unusable(format!(
                "merge time `{merged_at}` after update time `{}`",
                listed.updated_at
            )));
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

    /// Every kept PR in sample order: merge time, then number, descending.
    /// Call only after the whole listing has been offered.
    pub fn into_sample(mut self) -> Vec<MergedPr> {
        self.pending
            .sort_by(|a, b| (&b.merged_at, b.number).cmp(&(&a.merged_at, a.number)));
        self.pending
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

/// Whether `path` is repository source: the public path-eligibility query
/// routes it to one of `languages` and, when `suffixes` is set, the path
/// ends with one of them.
pub fn eligible(
    symbolic: &SymbolicContext,
    languages: &[Language],
    suffixes: Option<&[&str]>,
    path: &RelativePath,
) -> bool {
    suffixes.is_none_or(|suffixes| suffixes.iter().any(|suffix| path.as_str().ends_with(suffix)))
        && symbolic
            .path_language(path.as_str())
            .is_ok_and(|language| language.is_some_and(|language| languages.contains(&language)))
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
    /// Fold one provider `page` into the listing; `last` marks the final
    /// page the caller will read. Accepted and rejected entries both count
    /// toward [`MAX_LISTED_FILES`]. An entry whose path is already listed,
    /// within this page or an earlier one, is rejected rather than kept, so
    /// `files` holds distinct paths in provider order.
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
}

/// A PR's changed source files, screened from its listing without snapshot
/// facts.
///
/// Invariant: `source` holds exactly the listing's eligible entries
/// in listing order, and `changed_lines` marks those whose patch parsed to
/// at least one head-side changed line.
#[derive(Debug, Clone)]
pub struct Screen {
    /// The PR's eligible source entries.
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
    /// Screen `listing` for PR `number`, keeping entries `eligible` accepts
    /// as the repository's source.
    ///
    /// # Errors
    ///
    /// [`SampleError::IncompleteListing`] for a truncated listing, then
    /// [`SampleError::RejectedFile`] for any rejected entry.
    pub fn new(
        number: u64,
        listing: Listing,
        eligible: impl Fn(&RelativePath) -> bool,
    ) -> Result<Screen, SampleError> {
        if listing.truncated {
            return Err(SampleError::IncompleteListing { number });
        }
        if let Some(rejected) = listing.rejected.into_iter().next() {
            return Err(SampleError::RejectedFile { number, rejected });
        }
        let mut screen = Screen {
            source: Vec::new(),
            changed_lines: Vec::new(),
            omitted: Vec::new(),
        };
        for file in listing.files {
            if !eligible(&file.path) {
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

/// Sampler contract checks: discovery ordering, listing
/// completeness, and PR qualification, all on in-memory inputs.
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

    /// Python or Rust source through the public path-eligibility query.
    fn language(path: &RelativePath) -> bool {
        eligible(
            &SymbolicContext::new(),
            &[Language::Python, Language::Rust],
            None,
            path,
        )
    }

    /// A one-hunk patch changing head-side line 1.
    const PATCH: &str = "@@ -1 +1 @@\n-a\n+b";

    /// Discovery keeps merged PRs before the cutoff on the base branch,
    /// merge time then number descending.
    #[test]
    fn discovery_orders_merged_prs() {
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
        let numbers: Vec<_> = discovery.into_sample().iter().map(|pr| pr.number).collect();
        assert_eq!(numbers, [5, 4, 3]);
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

    /// A TypeScript repository's source is what the public query routes to
    /// TypeScript or TSX with a `.ts` or `.tsx` suffix, so `.mts`, `.cts`,
    /// JavaScript, Python, and docs screen out and never count toward
    /// qualification.
    #[test]
    fn typescript_eligibility_uses_public_query() {
        let symbolic = SymbolicContext::new();
        let screen = |files: Vec<PullRequestFile>| {
            Screen::new(
                5,
                Listing {
                    files,
                    ..Listing::default()
                },
                |path| {
                    eligible(
                        &symbolic,
                        &[Language::Typescript, Language::Tsx],
                        Some(&[".ts", ".tsx"]),
                        path,
                    )
                },
            )
            .expect("complete listing")
        };
        let modified = |path: &str| file(path, FileStatus::Modified, Some(PATCH));
        let mixed = screen(vec![
            modified("src/a.ts"),
            modified("src/b.tsx"),
            modified("src/c.mts"),
            modified("src/d.cts"),
            modified("src/e.js"),
            modified("src/f.py"),
            modified("README.md"),
        ]);
        let paths: Vec<_> = mixed.source.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/a.ts", "src/b.tsx"]);
        assert!(mixed.qualifies(|_| true));
        assert!(!screen(vec![modified("src/a.ts"), modified("src/c.mts")]).plausible());
        let mut many: Vec<_> = (0..15).map(|i| modified(&format!("src/f{i}.ts"))).collect();
        many.push(modified("src/g.cts"));
        assert!(screen(many).plausible());
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

    /// A listing folded from provider pages is incomplete when a page past
    /// the last is advertised or when accepted plus rejected entries reach
    /// the provider's file ceiling, even with no next page; a shorter
    /// terminal page is complete.
    #[test]
    fn listing_fails_at_provider_limits() {
        let page =
            |n: usize, accepted: usize, rejected: usize, has_next: bool| PullRequestFilesPage {
                files: (0..accepted)
                    .map(|i| {
                        file(
                            &format!("docs/p{n}/f{i}.md"),
                            FileStatus::Modified,
                            Some(PATCH),
                        )
                    })
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
            Screen::new(3, listing, language).err()
        };
        let capped = (1..=30).map(|n| page(n, 99, 1, n < 30)).collect();
        assert_eq!(
            fold(capped),
            Some(SampleError::IncompleteListing { number: 3 })
        );
        let advertised = (1..=30).map(|n| page(n, 50, 0, true)).collect();
        assert_eq!(
            fold(advertised),
            Some(SampleError::IncompleteListing { number: 3 })
        );
        assert_eq!(
            fold(vec![page(1, 100, 0, true), page(2, 40, 0, false)]),
            None
        );
    }

    /// A canonical path listed twice, within one page or across pages and
    /// whatever its status or patch, is a rejected entry that fails the run
    /// before qualification; distinct paths still qualify.
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
            Screen::new(4, listing, language)
        };
        let repeated = |result: Result<Screen, SampleError>| match result {
            Err(SampleError::RejectedFile { number, rejected }) => {
                number == 4 && rejected.path == "src/a.py"
            }
            _ => false,
        };
        let first = file("src/a.py", FileStatus::Modified, Some(PATCH));
        let again = file("src/a.py", FileStatus::Added, None);
        assert!(repeated(fold(vec![vec![first.clone(), again.clone()]])));
        assert!(repeated(fold(vec![vec![first.clone()], vec![again]])));
        let distinct = fold(vec![
            vec![first],
            vec![file("src/b.py", FileStatus::Added, Some(PATCH))],
        ])
        .expect("distinct paths");
        assert_eq!(distinct.source.len(), 2);
        assert!(distinct.qualifies(|_| true));
    }

    /// Cutoff, update, and merge times must name real Gregorian UTC
    /// instants; a real leap day is accepted and ordering is unchanged.
    #[test]
    fn discovery_rejects_impossible_timestamps() {
        let impossible = [
            "2026-02-30T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-00-10T00:00:00Z",
            "2026-13-10T00:00:00Z",
            "2026-01-00T00:00:00Z",
            "2026-01-10T24:00:00Z",
            "2026-01-10T00:60:00Z",
            "2026-01-10T00:00:60Z",
            "2026-02-30T25:61:61Z",
        ];
        for text in impossible {
            assert_eq!(
                Discovery::new("main", text).err(),
                Some(SampleError::InvalidCutoff(text.to_owned())),
                "cutoff {text}"
            );
            let mut discovery =
                Discovery::new("main", "2030-01-01T00:00:00Z").expect("valid cutoff");
            assert!(
                matches!(
                    discovery.offer(listed(9, text, None)),
                    Err(SampleError::UnusablePr { number: 9, .. })
                ),
                "update {text}"
            );
            assert!(
                matches!(
                    discovery.offer(listed(10, "2026-01-10T00:00:00Z", Some(text))),
                    Err(SampleError::UnusablePr { number: 10, .. })
                ),
                "merge {text}"
            );
        }
        let mut discovery = Discovery::new("main", "2024-02-29T23:59:59Z").expect("leap day");
        for pr in [
            listed(11, "2024-03-01T00:00:00Z", Some("2024-02-29T12:00:00Z")),
            listed(12, "2024-03-01T00:00:00Z", Some("2024-02-29T12:00:00Z")),
            listed(13, "2024-03-01T00:00:00Z", Some("2000-02-29T23:59:59Z")),
        ] {
            discovery.offer(pr).expect("real instants");
        }
        let numbers: Vec<_> = discovery.into_sample().iter().map(|pr| pr.number).collect();
        assert_eq!(numbers, [12, 11, 13]);
    }

    /// A merge after the PR's last update is impossible provider data and
    /// stops the run before the PR can be kept; equal times and an update
    /// after merge are kept in the usual order.
    #[test]
    fn discovery_rejects_merge_after_update() {
        let mut discovery = Discovery::new("main", "2026-01-10T00:00:00Z").expect("valid cutoff");
        assert!(matches!(
            discovery.offer(listed(
                14,
                "2026-01-05T00:00:00Z",
                Some("2026-01-06T00:00:00Z")
            )),
            Err(SampleError::UnusablePr { number: 14, .. })
        ));
        for pr in [
            listed(15, "2026-01-06T00:00:00Z", Some("2026-01-06T00:00:00Z")),
            listed(16, "2026-01-06T00:00:00Z", Some("2026-01-04T00:00:00Z")),
        ] {
            discovery.offer(pr).expect("consistent chronology");
        }
        let numbers: Vec<_> = discovery.into_sample().iter().map(|pr| pr.number).collect();
        assert_eq!(numbers, [15, 16]);
    }
}
