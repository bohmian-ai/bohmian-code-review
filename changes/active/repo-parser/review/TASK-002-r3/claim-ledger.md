# TASK-002 r3 — claim ledger (orchestrator-built, citation-checked)

Candidate dbd6de34f49764b32b00628cc539e719914469ed, base 1842eafdbc884d29f45fcee1c952ed3c89dd3e15. All 25 citations OK (`citations.txt`). No Wave 1 reasoning, matrices, or overall results are included.

## Prior FIND-* ledger (from r1/r2; remediation context)

| ID | Prior status | Root cause (prior) | Prior siblings |
|---|---|---|---|
| FIND-TASK-002-1 | CLASS_INCOMPLETE at r2 (fixed in 5cfa554) | ObjectKey/ContextId not validated at the durable seam (r1); then parse used a 100-byte name validator that refused constructor-built keys (r2) | storage.rs parse/Deserialize, persistence.rs SnapshotPointer.key / ContextRecord.key, identity.rs validators |
| FIND-TASK-002-2 | CLASS_INCOMPLETE at r2 (fixed in ebb3869) | fallible items lacking `# Errors` | github.rs BoundedReader::read |
| FIND-TASK-002-3 | CLOSED | speculative ObjectKind | none |
| FIND-TASK-002-4 | CONFIRMED at r2 (fixed in ebb3869) | GitHubHost::Enterprise { api_base } publicly constructible without validation; archive_host unchecked slice | identity.rs variant, archive_host; github.rs request/build/is_trusted_redirect |

r2 recorded (as a non-blocking note, r1-accepted scope): "well-formed foreign-scope keys still deserialize; scope binding is at facade lookup under Identity; change review should confirm the facades bind it."

## Claims

| Source ID | Reviewer | Claim (one sentence) | Root cause | Defect class | Sibling locations | Citations |
|---|---|---|---|---|---|---|
| TASK-A-1 | task-rev A | `ObjectKey` reload (parse/Deserialize) accepts unscoped (`"x"`) and foreign-tenant keys, so a persisted `SnapshotPointer`/`ContextRecord` can name another tenant's object under the caller's identity (claimed FIND-TASK-002-1 CLASS_INCOMPLETE; cites spec INV-007 spec.md:91, REQ-014 spec.md:77, AGENTS.md:29). | `ObjectKey::parse` validates segment alphabet only and never re-establishes the scope prefix the constructors emit | reload invariant weaker than construction invariant | storage.rs:94 (parse), :118 (Deserialize), :3-5/:44-45 (docs); persistence.rs:51, :110 (key fields), :205 (remove_repository returns keys) | storage.rs:94, :118, :3; persistence.rs:51, :110, :205 |
| TASK-A-2 | task-rev A | The `compile_fail` doctest that is the only proof of AC-R2-5 never runs under `test:pipeline`/`test:rust`/`gate` because the lane uses `--all-targets`, which excludes doctests (cites AGENTS.md:40, testing-workflows.md:15). | mise.toml:56 lane flags | verification target absent from owning lane | mise.toml:56; identity.rs:310 | mise.toml:56; identity.rs:310 |
| TASK-B-1 | task-rev B | Same claim as TASK-A-2 (DRIFT classification): removing `#[non_exhaustive]` leaves gate green. | mise.toml:56 | same | mise.toml:56; identity.rs:310 | same |
| REPO-1 | repo-rev | Two caller-input panics are undocumented: `Execution::with_timeout(Duration::MAX)` panics on `Instant + Duration`; `read_bounded` computes `limit as u64 + 1`, which overflows for `usize::MAX` under overflow-checks (cites AGENTS.md:32, agent-rules.md:11, rust-core.md:7). | unchecked arithmetic on caller-supplied bounds | undocumented panic on input | execution.rs:59; github.rs:1273 (callers :934, :1066, :1104) | execution.rs:59; github.rs:1273 |
| REPO-2 | repo-rev | Every `mod tests` in the crate lacks rustdoc while core crates document theirs (cites AGENTS.md:32). | module wrapper not documented | rustdoc omission on new item | execution.rs:87; identity.rs:412; github.rs:1367; persistence.rs:209; storage.rs:184 | same |
| REPO-3 | repo-rev | persistence.rs uses `std::fmt::Display`/`Formatter`/`Result` path-qualified in signatures while siblings import `fmt` (cites AGENTS.md:31, agent-rules.md:11). | missing `use std::fmt` | import discipline | persistence.rs:97, :99 (also `serde::Deserializer`/`serde::de::Error` sites mirroring core code, recorded for completeness) | persistence.rs:97, :99 |
| TRUST-1 | domain-rev source-trust | `#[non_exhaustive]` blocks external construction but not mutation: external code binding `GitHubHost::Enterprise { api_base, .. } = &mut h` can assign `"http://evil"`, so the bearer token is sent to an arbitrary cleartext host and `archive_host` degrades to `""` (cites identity.rs:307-308 doc, spec REQ-014 spec.md:77, REQ-004 spec.md:61, AGENTS.md:29). Scratch crate compiled and printed forged base. | enum variant field is inherently `pub`; `#[non_exhaustive]` does not close the write path | validated domain value with public writable field (same class as FIND-TASK-002-4) | identity.rs:318-322 (variant), :353-358 (api_base), :361-369 (archive_host); github.rs:1121 (request), :1139 (build), :741-746 (is_trusted_redirect) | identity.rs:321, :307, :318, :367; github.rs:1121, :1139 |
| — | domain-rev provider-bounds | No findings. | — | — | — | [] |
