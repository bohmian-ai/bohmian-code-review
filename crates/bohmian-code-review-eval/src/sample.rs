//! PR sample selection for the context-relevance evaluation.
//!
//! Discovery reads every PR oldest-created first through the pipeline's
//! verified [`ListedPullRequest`] pages before qualifying any, so
//! activity during the read can only append to the listing, never shift a
//! page already read. Merged PRs are then ordered by merge time descending,
//! then PR number descending. Each is qualified in that order from its one
//! file listing, its repository's source-language eligibility, and its base
//! snapshot's declarations. Missing or malformed patches are file-local
//! omissions; duplicate PRs, rejected entries, and incomplete listings
//! fail the run. Listings and merged-PR identities are the
//! pipeline's history types.

use bohmian_code_review::{Language, RelativePath, SymbolicContext, parse_hunks};
use bohmian_code_review_pipeline::{
    FileStatus, HistoryError, ListedPullRequest, Listing, MergedPr, ObjectSha, PullRequestFile,
    is_timestamp,
};
use std::collections::BTreeSet;
use thiserror::Error;

/// PRs sampled per repository.
pub const SAMPLE_SIZE: usize = 50;
/// Fewest changed source files a sampled PR may have.
const MIN_SOURCE_FILES: usize = 2;
/// Most changed source files a sampled PR may have.
const MAX_SOURCE_FILES: usize = 15;

/// Why sampling cannot produce a trustworthy sample; the run stops.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SampleError {
    /// The PR listing named one PR twice, so pages shifted mid-read.
    #[error("PR #{number} listed twice; the PR listing changed while it was read")]
    DuplicatePr {
        /// Repeated PR number.
        number: u64,
    },
    /// The configured cutoff is not a `YYYY-MM-DDTHH:MM:SSZ` timestamp.
    #[error("cutoff `{0}` is not a YYYY-MM-DDTHH:MM:SSZ timestamp")]
    InvalidCutoff(String),
    /// A PR listing or the history it feeds cannot be trusted.
    #[error(transparent)]
    History(#[from] HistoryError),
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

    /// Record one verified listed PR, keeping it only when it merged into
    /// the branch before the cutoff.
    ///
    /// # Errors
    ///
    /// [`SampleError::DuplicatePr`] for a number already offered.
    pub fn offer(&mut self, listed: ListedPullRequest) -> Result<(), SampleError> {
        if !self.seen.insert(listed.number) {
            return Err(SampleError::DuplicatePr {
                number: listed.number,
            });
        }
        let Some(merged_at) = listed.merged_at else {
            return Ok(());
        };
        if listed.base_branch != self.branch || merged_at >= self.cutoff {
            return Ok(());
        }
        self.pending.push(MergedPr {
            number: listed.number,
            base: listed.base,
            head: listed.head,
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

/// Whether `path` is repository source: the public path-eligibility query
/// routes it to one of `languages` and, when `suffixes` is set, the path
/// ends with one of them.
pub fn eligible(
    symbolic: &SymbolicContext,
    languages: &[Language],
    suffixes: Option<&[&str]>,
    path: &RelativePath,
) -> bool {
    suffixes.is_none_or(|suffixes| {
        suffixes
            .iter()
            .any(|suffix| path.as_str().ends_with(suffix))
    }) && symbolic
        .path_language(path.as_str())
        .is_ok_and(|language| language.is_some_and(|language| languages.contains(&language)))
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
    /// [`SampleError::History`] with the error of [`Listing::complete`].
    pub fn new(
        number: u64,
        listing: Listing,
        eligible: impl Fn(&RelativePath) -> bool,
    ) -> Result<Screen, SampleError> {
        listing.complete(number)?;
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
    use bohmian_code_review_pipeline::{PullRequestFilesPage, RejectedPath};

    /// A complete one-page listing of `files`.
    fn listing(files: Vec<PullRequestFile>) -> Listing {
        let mut listing = Listing::default();
        let page = PullRequestFilesPage {
            files,
            rejected: Vec::new(),
            has_next: false,
        };
        listing.push(page, true);
        listing
    }

    /// A listed PR merged into `main` at `merged_at` (or unmerged).
    fn listed(number: u64, updated_at: &str, merged_at: Option<&str>) -> ListedPullRequest {
        let sha = |c: &str| ObjectSha::new(&c.repeat(40)).expect("fixture sha is valid");
        ListedPullRequest {
            number,
            updated_at: updated_at.to_owned(),
            merged_at: merged_at.map(str::to_owned),
            base_branch: "main".to_owned(),
            base: sha("a"),
            head: sha("b"),
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
        other.base_branch = "release".to_owned();
        discovery.offer(other).expect("usable PR");
        let numbers: Vec<_> = discovery.into_sample().iter().map(|pr| pr.number).collect();
        assert_eq!(numbers, [5, 4, 3]);
    }

    /// Duplicates and a bad cutoff stop the run; unusable numbers, times,
    /// and commits are rejected earlier by the pipeline's PR listing.
    #[test]
    fn discovery_fails_on_unusable_input() {
        assert_eq!(
            Discovery::new("main", "2026-01-10").err(),
            Some(SampleError::InvalidCutoff("2026-01-10".to_owned()))
        );
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
            Screen::new(1, listing(files), language).expect("complete listing")
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
            Screen::new(5, listing(files), |path| {
                eligible(
                    &symbolic,
                    &[Language::Typescript, Language::Tsx],
                    Some(&[".ts", ".tsx"]),
                    path,
                )
            })
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

    /// A rejected entry or a truncated listing fails the run with the
    /// pipeline's history error.
    #[test]
    fn screen_fails_on_rejected_or_incomplete_listing() {
        let rejected = RejectedPath {
            path: "../x.py".to_owned(),
            reason: "not canonical".to_owned(),
        };
        let mut listing = Listing::default();
        listing.rejected.push(rejected.clone());
        assert_eq!(
            Screen::new(3, listing, language).err(),
            Some(SampleError::History(HistoryError::RejectedFile {
                number: 3,
                rejected
            }))
        );
        let mut listing = Listing::default();
        listing.truncated = true;
        assert_eq!(
            Screen::new(3, listing, language).err(),
            Some(SampleError::History(HistoryError::IncompleteListing {
                number: 3
            }))
        );
    }

    /// The cutoff must name a real Gregorian UTC instant; a real leap day is
    /// accepted and ordering is unchanged.
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
}
