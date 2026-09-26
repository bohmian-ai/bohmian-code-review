//! Public-API symbolic-context journey over in-memory TypeScript.
//!
//! Drives only `bohmian-code-review`'s facade through the real extractor,
//! index, resolver, and selector: upsert supplied blobs, rank a changed
//! handler, and pack context under an exact caller-counted token budget.

use bohmian_code_review::{
    ContextLimits, Error, ExtractionError, LineRange, OmissionReason, RelativePath, SelectError,
    SpanKind, SymbolicContext, build_context,
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

/// All four journey files.
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

#[test]
/// The two-pass journey ranks user above format, excludes unrelated, packs
/// one secondary span under an exact budget, and repeats identically.
fn changed_handler_ranks_and_packs_user_context_deterministically() {
    let context = indexed();
    let changed = [LineRange { start: 5, end: 8 }];
    let ranking = context
        .rank("src/handler.ts", HANDLER.as_bytes(), changed.to_vec())
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

    let again = context
        .rank("src/handler.ts", HANDLER.as_bytes(), changed.to_vec())
        .expect("valid change");
    assert_eq!(again, ranking);
    assert_eq!(
        build_context(&again, HANDLER.as_bytes(), &candidates, limits, words),
        Ok(packed)
    );
}

#[test]
/// An unfittable changed hunk, a missing candidate blob, and deletion-only
/// input fail explicitly through the public error.
fn journey_reports_budget_missing_blob_and_deletion_only_errors() {
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
