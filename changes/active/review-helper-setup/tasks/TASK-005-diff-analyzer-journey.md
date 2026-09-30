---
id: CR-SETUP-T05
kind: implementation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 3
requirements: [REQ-011, REQ-007, INV-001, INV-003, AC-005]
depends_on: [CR-SETUP-T03]
---

# Diff analysis and diagram-complete journey

## Outcome and Value

Callers hand the library PR diff text instead of precomputed line ranges, and the public journey covers CodePal's full "Building Code Context" flow: parse source into the symbol-to-file index, analyze the PR diff, look up symbols, score files by overlap, and pick context within the token budget. Required execution skill: `$cr-implement`.

## Owners and Scope

`bohmian-code-review-select` owns diff analysis next to `LineRange`; the facade re-exports it and converts `DiffError`. The external journey starts from diff text. No IO, no Git provider calls, no new crate or dependency.

## Acceptance Criteria

- REQ-011 behavior with typed, line-numbered failures for corrupt diffs.
- AC-005 journey derives the handler's changed lines from PR diff text, skips a deleted file, and rejects a truncated diff through the public error.

## Implementation Evidence

RED: diff tests compiled against a missing API (`cannot find function parse_unified_diff`, `FileDiff`, `DiffError`). GREEN on first implementation.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Multi-file Git and plain diffs yield per-file paths and coalesced head-side added ranges | `select/src/diff.rs` `parse_unified_diff` | `git_diff_reports_added_ranges_per_file` (modify with blank context and a `---`-looking removed line, add, delete, rename with spaces, binary, quoted non-ASCII path, commit text and format-patch trailer); `plain_unified_diff_splits_on_file_headers` | PASS |
| Hunk-only provider patches | `parse_hunks` | `provider_patch_reports_coalesced_added_ranges` | PASS |
| Corrupt input fails with the diff line | `DiffError` | `corrupt_diffs_fail_explicitly`: truncated, overlong, unknown prefix, unordered, malformed header, headerless hunk, hunk cut by next file, `../` path, bad escape, missing path | PASS |
| Journey starts from PR diff text | `tests/symbolic_context_journey.rs` `PR_DIFF`, `changed_files` | `pr_diff_selects_user_context_deterministically`: handler 5–8, deleted `src/legacy.ts` skipped, prior ranking/packing assertions unchanged, rerun from diff identical; truncated diff → `Error::Diff(HunkLengthMismatch { line: 20 })` | PASS |
| No IO; facade has no duplicate logic | facade re-exports only, `Error::Diff` via `#[from]` | `rg std::(fs|net|process)` in select empty | PASS |

Verification: `mise run gate` exit 0 (core 25, extract 138 + 1, index 3, resolve 1, select 8, journey 3; fmt, Clippy `-D warnings`, workspace check, skill sync); `git diff --check` clean.

Decisions: only default `a/`/`b/` prefixes are stripped (`ponytail:` note in `side_path`); combined merge diffs (`@@@`) are rejected as malformed hunk headers; binary files are reported like renames, with no added lines.
