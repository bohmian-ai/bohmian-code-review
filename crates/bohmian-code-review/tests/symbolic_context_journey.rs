//! Public-API symbolic-context journey over in-memory TypeScript.
//!
//! Follows CodePal's "Building Code Context" flow through only
//! `bohmian-code-review`'s facade and the real extractor, index, resolver,
//! and selector: parse source code into the symbol-to-file index (pass one),
//! analyze the PR diff, then look up symbols, score files by overlap, and
//! pick context within an exact caller-counted token budget (pass two).

use bohmian_code_review::{
    ChangedBlob, ContextLimits, DiffError, EXTRACTION_VERSION, Error, ExtractionError, FileDiff,
    IndexedFileFacts, Language, LineRange, OmissionReason, RelativePath, SelectError, SpanKind,
    SymbolicContext, build_candidate_context, build_context, parse_unified_diff,
};
use std::collections::BTreeMap;

/// Changed handler; lines 5..=8 call `lookupUser`, `saveUser` twice, and
/// `formatUser`.
const HANDLER: &str = "import { lookupUser, saveUser } from \"./user\";
import { formatUser } from \"./format\";

export function handleRequest(id: string): string {
  const user = lookupUser(id);
  saveUser(user);
  saveUser(user);
  return formatUser(user);
}
";

/// Declares `lookupUser` (lines 5..=7) and `saveUser` (lines 9..=11).
const USER: &str = "export interface User {
  id: string;
}

export function lookupUser(id: string): User {
  return { id };
}

export function saveUser(user: User): void {
  console.log(user.id);
}
";

/// Declares `formatUser`.
const FORMAT: &str = "import { User } from \"./user\";

export function formatUser(user: User): string {
  return user.id;
}
";

/// Declares nothing the handler uses.
const UNRELATED: &str = "export function unrelated(): number {
  return 1;
}
";

/// The PR diff: rewrites the handler body to call the user and format
/// helpers, and deletes a legacy file that no longer exists at head.
const PR_DIFF: &str = concat!(
    "diff --git a/src/handler.ts b/src/handler.ts\n",
    "index 1111111..2222222 100644\n",
    "--- a/src/handler.ts\n",
    "+++ b/src/handler.ts\n",
    "@@ -2,5 +2,8 @@ import { lookupUser, saveUser } from \"./user\";\n",
    " import { formatUser } from \"./format\";\n",
    " \n",
    " export function handleRequest(id: string): string {\n",
    "-  return id;\n",
    "+  const user = lookupUser(id);\n",
    "+  saveUser(user);\n",
    "+  saveUser(user);\n",
    "+  return formatUser(user);\n",
    " }\n",
    "diff --git a/src/legacy.ts b/src/legacy.ts\n",
    "deleted file mode 100644\n",
    "index 3333333..0000000\n",
    "--- a/src/legacy.ts\n",
    "+++ /dev/null\n",
    "@@ -1,3 +0,0 @@\n",
    "-export function legacy(): void {\n",
    "-  return;\n",
    "-}\n",
);

/// All four head-side journey files.
const FILES: [(&str, &str); 4] = [
    ("src/handler.ts", HANDLER),
    ("src/user.ts", USER),
    ("src/format.ts", FORMAT),
    ("src/unrelated.ts", UNRELATED),
];

/// Counts whitespace-separated words, standing in for a model tokenizer.
fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Parses a path known to be canonical.
///
/// # Panics
///
/// Panics if `raw` is not a valid repository-relative path.
fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

/// Upserts every journey file from a buffer dropped right after the call.
///
/// # Panics
///
/// Panics if a journey file fails extraction.
fn indexed() -> SymbolicContext {
    let mut context = SymbolicContext::new();
    for (file, source) in FILES {
        let bytes = source.as_bytes().to_vec();
        context.upsert(file, &bytes).expect("journey file extracts");
    }
    context
}

/// The caller's candidate blob map for `files`.
fn blobs(files: &[&str]) -> BTreeMap<RelativePath, &'static [u8]> {
    FILES
        .iter()
        .filter(|(file, _)| files.contains(file))
        .map(|&(file, source)| (path(file), source.as_bytes()))
        .collect()
}

#[test]
/// Upsert returns extracted facts and byte length; a rejected replacement
/// keeps the previous facts, and removal is idempotent.
fn upsert_is_transactional_and_remove_is_idempotent() {
    let mut context = SymbolicContext::new();
    let facts = context
        .upsert("src/user.ts", USER.as_bytes())
        .expect("extracts");
    assert_eq!(facts.byte_len, USER.len());
    let names: Vec<&str> = facts.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, ["user.ts", "User", "lookupUser", "saveUser"]);
    assert!(
        facts
            .references
            .iter()
            .any(|reference| reference.reference_name == "console.log")
    );
    assert!(facts.diagnostics.is_empty());
    context
        .upsert("src/unrelated.ts", UNRELATED.as_bytes())
        .expect("extracts");

    let malformed = context.upsert("src/user.ts", b"export function (((\n");
    assert!(matches!(
        malformed,
        Err(Error::Extraction { ref path, source: ExtractionError::Deferred { .. } })
            if path.as_str() == "src/user.ts"
    ));
    assert!(matches!(
        context.upsert("src/user.ts", b"\xff"),
        Err(Error::NotUtf8 { .. })
    ));
    assert!(matches!(
        context.upsert("../user.ts", USER.as_bytes()),
        Err(Error::InvalidPath { .. })
    ));
    assert!(matches!(
        context.upsert("docs/blob.bin", b"\x00\x01"),
        Err(Error::Extraction {
            source: ExtractionError::UnsupportedLanguage { .. },
            ..
        })
    ));
    assert_eq!(context.file_count(), 2);
    assert_eq!(context.byte_len(&path("src/user.ts")), Some(USER.len()));
    assert_eq!(context.lookup("lookupUser")[0].path.as_str(), "src/user.ts");
    assert_eq!(context.lookup("unrelated").len(), 1);

    assert_eq!(context.remove("src/unrelated.ts"), Ok(true));
    assert_eq!(context.remove("src/unrelated.ts"), Ok(false));
    assert!(context.lookup("unrelated").is_empty());
    assert_eq!(context.file_count(), 1);
}

/// Analyzes the PR diff and returns the head path and added lines of every
/// file with head-side changes to rank.
///
/// # Panics
///
/// Panics if the diff is malformed or names a non-canonical path.
fn changed_files(diff: &str) -> Vec<(String, Vec<LineRange>)> {
    parse_unified_diff(diff)
        .expect("valid diff")
        .into_iter()
        .filter_map(|file| Some((file.new_path?.as_str().to_owned(), file.added)))
        .filter(|(_, added)| !added.is_empty())
        .collect()
}

#[test]
/// The diagram's full flow: the PR diff yields the handler's changed lines,
/// ranking scores user above format and excludes unrelated, packing picks
/// one secondary span under an exact budget, and a rerun is identical.
fn pr_diff_selects_user_context_deterministically() {
    let context = indexed();
    assert_eq!(
        parse_unified_diff(PR_DIFF),
        Ok(vec![
            FileDiff {
                old_path: Some(path("src/handler.ts")),
                new_path: Some(path("src/handler.ts")),
                added: vec![LineRange { start: 5, end: 8 }],
            },
            FileDiff {
                old_path: Some(path("src/legacy.ts")),
                new_path: None,
                added: Vec::new(),
            },
        ])
    );
    let changed = changed_files(PR_DIFF);
    assert_eq!(
        changed,
        [(
            "src/handler.ts".to_owned(),
            vec![LineRange { start: 5, end: 8 }]
        )]
    );
    let (changed_path, changed_lines) = &changed[0];
    let ranking = context
        .rank(changed_path, HANDLER.as_bytes(), changed_lines.clone())
        .expect("valid change");

    let scores: Vec<(&str, usize)> = ranking
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.score))
        .collect();
    assert_eq!(scores, [("src/user.ts", 2), ("src/format.ts", 1)]);
    let reasons: Vec<(&str, u32, u32)> = ranking.files[0]
        .reasons
        .iter()
        .map(|reason| (reason.name.as_str(), reason.lines.start, reason.lines.end))
        .collect();
    assert_eq!(reasons, [("lookupUser", 5, 7), ("saveUser", 9, 11)]);

    let limits = ContextLimits {
        max_files: 2,
        max_tokens: 19,
    };
    let candidates = blobs(&["src/user.ts"]);
    let packed = build_context(&ranking, HANDLER.as_bytes(), &candidates, limits, words)
        .expect("fits budget");
    let spans: Vec<(&str, u32, u32, &str, usize, &SpanKind)> = packed
        .spans
        .iter()
        .map(|span| {
            (
                span.path.as_str(),
                span.lines.start,
                span.lines.end,
                span.text.as_str(),
                span.tokens,
                &span.kind,
            )
        })
        .collect();
    let user_reason = SpanKind::Candidate {
        score: 2,
        names: vec!["lookupUser".to_owned()],
    };
    assert_eq!(
        spans,
        [
            (
                "src/handler.ts",
                5,
                8,
                "  const user = lookupUser(id);\n  saveUser(user);\n  saveUser(user);\n  return formatUser(user);\n",
                8,
                &SpanKind::ChangedHunk,
            ),
            (
                "src/user.ts",
                5,
                7,
                "export function lookupUser(id: string): User {\n  return { id };\n}\n",
                11,
                &user_reason,
            ),
        ]
    );
    assert_eq!((packed.files, packed.tokens), (2, 19));
    let omissions: Vec<(&str, u32, OmissionReason)> = packed
        .omissions
        .iter()
        .map(|omission| {
            (
                omission.path.as_str(),
                omission.lines.start,
                omission.reason,
            )
        })
        .collect();
    assert_eq!(
        omissions,
        [
            ("src/user.ts", 9, OmissionReason::TokenLimit { tokens: 8 }),
            ("src/format.ts", 3, OmissionReason::FileLimit),
        ]
    );

    let (again_path, again_lines) = &changed_files(PR_DIFF)[0];
    let again = context
        .rank(again_path, HANDLER.as_bytes(), again_lines.clone())
        .expect("valid change");
    assert_eq!(again, ranking);
    assert_eq!(
        build_context(&again, HANDLER.as_bytes(), &candidates, limits, words),
        Ok(packed)
    );
}

#[test]
/// A truncated PR diff, an unfittable changed hunk, a missing candidate blob,
/// and deletion-only input fail explicitly through the public error.
fn journey_reports_budget_missing_blob_and_deletion_only_errors() {
    let truncated = PR_DIFF
        .strip_suffix("-}\n")
        .expect("diff ends with the deleted brace");
    assert_eq!(
        parse_unified_diff(truncated).map_err(Error::from),
        Err(Error::Diff(DiffError::HunkLengthMismatch { line: 20 }))
    );

    let context = indexed();
    let ranking = context
        .rank(
            "src/handler.ts",
            HANDLER.as_bytes(),
            vec![LineRange { start: 5, end: 8 }],
        )
        .expect("valid change");
    let tight = ContextLimits {
        max_files: 2,
        max_tokens: 7,
    };
    assert_eq!(
        build_context(&ranking, HANDLER.as_bytes(), &blobs(&[]), tight, words),
        Err(Error::Select(SelectError::ChangedHunkOverBudget {
            lines: LineRange { start: 5, end: 8 },
            tokens: 8,
            remaining_tokens: 7,
        }))
    );
    let roomy = ContextLimits {
        max_files: 3,
        max_tokens: 1_000,
    };
    assert_eq!(
        build_context(
            &ranking,
            HANDLER.as_bytes(),
            &blobs(&["src/user.ts"]),
            roomy,
            words
        ),
        Err(Error::Select(SelectError::MissingBlob {
            path: path("src/format.ts")
        }))
    );
    assert_eq!(
        context.rank("src/handler.ts", HANDLER.as_bytes(), Vec::new()),
        Err(Error::Select(SelectError::DeletionOnly))
    );
}

/// Changed second file; line 3 calls `formatUser` and `unrelated`.
const REPORT: &str = "import { formatUser } from \"./format\";

export const report = (user: User) => formatUser(user) + unrelated();
";

#[test]
/// Exported facts are source-free and version-stamped; loading them into a
/// fresh context reproduces lookup, ranking, and packed candidate spans,
/// while malformed or foreign-version facts fail without touching other
/// loaded files.
fn exported_facts_reload_without_source_and_reject_invalid_records() {
    let context = indexed();
    let exported = context.export();
    let paths: Vec<&str> = exported.iter().map(|facts| facts.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "src/format.ts",
            "src/handler.ts",
            "src/unrelated.ts",
            "src/user.ts"
        ]
    );
    let user = &exported[3];
    assert_eq!(user.byte_len, USER.len());
    assert_eq!(user.extraction_version, EXTRACTION_VERSION);
    let declared: Vec<(&str, u32, u32)> = user
        .declarations
        .iter()
        .map(|fact| (fact.name.as_str(), fact.start_line, fact.end_line))
        .collect();
    assert_eq!(
        declared,
        [("User", 1, 3), ("lookupUser", 5, 7), ("saveUser", 9, 11)]
    );
    let json = serde_json::to_string(&exported).expect("facts serialize");
    for fragment in [
        "console.log",
        "return { id }",
        "(id: string)",
        "docstring",
        "signature",
    ] {
        assert!(
            !json.contains(fragment),
            "source fragment leaked: {fragment}"
        );
    }

    let mut loaded = SymbolicContext::new();
    for facts in serde_json::from_str::<Vec<IndexedFileFacts>>(&json).expect("facts parse") {
        loaded.load(facts).expect("valid facts");
    }
    assert_eq!(loaded.file_count(), 4);
    assert_eq!(loaded.byte_len(&path("src/user.ts")), Some(USER.len()));
    assert_eq!(loaded.lookup("lookupUser")[0].path.as_str(), "src/user.ts");
    assert!(loaded.lookup("console.log").is_empty());
    let changed = vec![LineRange { start: 5, end: 8 }];
    let fresh = context
        .rank("src/handler.ts", HANDLER.as_bytes(), changed.clone())
        .expect("valid change");
    let reloaded = loaded
        .rank("src/handler.ts", HANDLER.as_bytes(), changed)
        .expect("valid change");
    assert_eq!(reloaded, fresh);
    let limits = ContextLimits {
        max_files: 2,
        max_tokens: 19,
    };
    let candidates = blobs(&["src/user.ts"]);
    assert_eq!(
        build_context(&reloaded, HANDLER.as_bytes(), &candidates, limits, words),
        build_context(&fresh, HANDLER.as_bytes(), &candidates, limits, words)
    );

    let mut stale = user.clone();
    stale.extraction_version = "other-extractor".to_owned();
    assert!(matches!(
        loaded.load(stale),
        Err(Error::ExtractionVersionMismatch { ref path, ref found, .. })
            if path.as_str() == "src/user.ts" && found == "other-extractor"
    ));
    let mut malformed = user.clone();
    malformed.declarations[1].start_line = 0;
    assert!(matches!(
        loaded.load(malformed),
        Err(Error::InvalidFacts { ref path, .. }) if path.as_str() == "src/user.ts"
    ));
    let mut renamed = user.clone();
    renamed.path = path("src/other.ts");
    renamed.declarations[0].name = String::new();
    assert!(matches!(
        loaded.load(renamed),
        Err(Error::InvalidFacts { ref path, .. }) if path.as_str() == "src/other.ts"
    ));
    assert_eq!(loaded.file_count(), 4);
    assert_eq!(loaded.lookup("lookupUser")[0].path.as_str(), "src/user.ts");
    assert!(
        serde_json::from_str::<IndexedFileFacts>(
            &json.replace("\"src/user.ts\"", "\"../user.ts\"")[1..json.len() - 1]
        )
        .is_err()
    );
}

#[test]
/// Path-only routing answers eligibility before download: native and
/// composite paths report their language, unregistered paths report none,
/// and non-canonical paths fail.
fn path_language_identifies_eligible_paths_before_download() {
    let context = SymbolicContext::new();
    assert_eq!(
        context.path_language("src/user.ts"),
        Ok(Some(Language::Typescript))
    );
    assert_eq!(
        context.path_language("lib/View.svelte"),
        Ok(Some(Language::Svelte))
    );
    assert_eq!(
        context.path_language("include/api.h"),
        Ok(Some(Language::C))
    );
    assert_eq!(context.path_language("README.md"), Ok(None));
    assert_eq!(context.path_language("Makefile"), Ok(None));
    assert!(matches!(
        context.path_language("/etc/passwd"),
        Err(Error::InvalidPath { .. })
    ));
    assert!(!EXTRACTION_VERSION.is_empty());
}

#[test]
/// Two changed files union their changed-line names, both changed paths are
/// excluded, `formatUser` scores once although both files reference it,
/// ties break by path, and candidate-only packing counts only selected
/// candidates while reporting file and token omissions.
fn multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only() {
    let mut context = indexed();
    context
        .upsert("src/report.ts", REPORT.as_bytes())
        .expect("extracts");
    let changed = [
        ChangedBlob {
            path: "src/handler.ts",
            head: HANDLER.as_bytes(),
            changed: vec![LineRange { start: 5, end: 8 }],
        },
        ChangedBlob {
            path: "src/report.ts",
            head: REPORT.as_bytes(),
            changed: vec![LineRange { start: 3, end: 3 }],
        },
    ];
    let ranked = context.rank_files(&changed).expect("valid changes");
    let scores: Vec<(&str, usize)> = ranked
        .iter()
        .map(|file| (file.path.as_str(), file.score))
        .collect();
    assert_eq!(
        scores,
        [
            ("src/user.ts", 2),
            ("src/format.ts", 1),
            ("src/unrelated.ts", 1)
        ]
    );
    assert!(
        ranked
            .iter()
            .all(|file| file.path.as_str() != "src/report.ts")
    );
    let single = context
        .rank(
            "src/handler.ts",
            HANDLER.as_bytes(),
            vec![LineRange { start: 5, end: 8 }],
        )
        .expect("valid change");
    assert_eq!(single.files[0], ranked[0]);

    let limits = ContextLimits {
        max_files: 2,
        max_tokens: 20,
    };
    let candidates = blobs(&["src/user.ts", "src/format.ts"]);
    let packed = build_candidate_context(&ranked, &candidates, limits, words).expect("packs");
    let spans: Vec<(&str, u32, u32, usize)> = packed
        .spans
        .iter()
        .map(|span| {
            (
                span.path.as_str(),
                span.lines.start,
                span.lines.end,
                span.tokens,
            )
        })
        .collect();
    assert_eq!(
        spans,
        [("src/user.ts", 5, 7, 11), ("src/format.ts", 3, 5, 9)]
    );
    assert!(
        packed
            .spans
            .iter()
            .all(|span| matches!(span.kind, SpanKind::Candidate { .. }))
    );
    assert_eq!((packed.files, packed.tokens), (2, 20));
    let omissions: Vec<(&str, u32, OmissionReason)> = packed
        .omissions
        .iter()
        .map(|omission| {
            (
                omission.path.as_str(),
                omission.lines.start,
                omission.reason,
            )
        })
        .collect();
    assert_eq!(
        omissions,
        [
            ("src/user.ts", 9, OmissionReason::TokenLimit { tokens: 8 }),
            ("src/unrelated.ts", 1, OmissionReason::FileLimit),
        ]
    );
    assert_eq!(
        build_candidate_context(&ranked, &candidates, limits, words),
        Ok(packed)
    );

    let none = ContextLimits {
        max_files: 3,
        max_tokens: 5,
    };
    let starved =
        build_candidate_context(&ranked, &blobs(&FILES.map(|(file, _)| file)), none, words)
            .expect("packs");
    assert!(starved.spans.is_empty());
    assert_eq!((starved.files, starved.tokens), (0, 0));
    assert_eq!(starved.omissions.len(), 4);
    assert!(
        starved
            .omissions
            .iter()
            .all(|omission| matches!(omission.reason, OmissionReason::TokenLimit { .. }))
    );
    assert!(matches!(
        context.rank_files(&[ChangedBlob {
            path: "src/report.ts",
            head: REPORT.as_bytes(),
            changed: Vec::new(),
        }]),
        Err(Error::Select(SelectError::DeletionOnly))
    ));
}
