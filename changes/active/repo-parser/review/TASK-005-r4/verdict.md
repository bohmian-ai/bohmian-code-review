# TASK-005 r4 verdict: FIX_REQUIRED

**Immutable subject:** base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `62346cb030354596cdf3dd7596de63dbd8d99fcb` (`HEAD` at review start and before verdict). Authority: approved `changes/active/repo-parser/spec.md` revision 12, original TASK-005, R1–R3 verdicts and remediation, and `review/TASK-005-r3/maintainer-decision-rejected-tree-entries.md`, which overrides R3's FIND-4 correction. The complete cumulative diff was reviewed without changing candidate source.

| Task acceptance obligation | Implementation and verification evidence | Result |
|---|---|---|
| S5–S7 / REQ-009 / INV-004: pinned, private head overlay and stale-base reconciliation | `PrContext::pin/run`, shared `Ingest`; concurrent-PR and stale-base tests, passing pipeline gate | PASS |
| S10 / REQ-008/013: bounded scoped index, eligible-file policy, honest coverage | `near`, changed-head `index_cap`, fallback and coverage tests pass; executable root policy is skipped, and actual PR blob size can bypass the configured file cap | **FAIL — FIND-TASK-005-8, FIND-TASK-005-9** |
| S18 / REQ-009/010: moving-PR retry, pagination, file-local omissions, changed-path exclusion | `pin`, `changed_head`, `rank`; moving-PR, rejected-path and broken-tree tests | PASS apart from oversized-file omission in FIND-8 |
| S19 / REQ-010 / INV-006: candidate-only budget and explicit exhaustion | `Budget`, `pack`; exhausted-budget test and passing symbolic journey | PASS apart from oversized candidate admission in FIND-8 |
| S14–S15 / REQ-011–015 / INV-005 / AC-009: tenant read, 24-hour expiry, interruption-safe publication and cleanup | `Contexts::publish/read/collect_expired`; focused in-flight-cleanup test and gate | PASS |
| R3 maintainer decision and FIND-6/7 closure | Provider errors for malformed tree mode/SHA, noncanonical paths as unsupported exclusions, scoped index cap, caller collection cutoff; focused tests and independent domain reviews | PASS |
| REQ-016/018 and non-goals: injected independent facades; no consumer/model/comment/server or symbolic-crate IO | Public pipeline API and cumulative diff; standards review and gate | PASS |

**Wave 1:** task A **FAIL** (TASK-A-1), task B **FAIL** (TASK-B-1), standards **PASS**, source trust **FAIL** (DOMAIN-TRUST-1), budget **PASS**, durability **PASS**. All required reports are present. The [citation check](citations.txt) accepted six citations for TASK-A-1 and DOMAIN-TRUST-1 and rejected all three TASK-B-1 citations because their symbol names did not satisfy the citation contract; TASK-B-1 was dropped. Its warm-reuse scenario was independently checked as a sibling of TASK-A-1. The [Wave 2 ledger](findings-validation.md) source-path reproduced the two retained claims, applied the blocking gate, and found no advisory notes.

| Validated finding | Status and gate | Consequence |
|---|---|---|
| FIND-TASK-005-8 | `CONFIRMED`; eligible-file definition, REQ-004/013, task contract | Missing or understated tree size can admit actual PR blobs above `max_file_bytes` into changed-head analysis/index, warm reused facts, or selected context. |
| FIND-TASK-005-9 | `CONFIRMED`; eligible-file definition, REQ-005/006/009, honest coverage | Executable root `.gitattributes` can be ignored, causing generated/vendored sources to be indexed and complete coverage claimed under default policy. |

**Prior findings:** FIND-TASK-005-1 through -3 and -5 through -7 remain closed by the existing object comparison, publication identity, neighbor classification, changed-path exclusion, shared scoped index cap, and safe context collection cutoff. FIND-TASK-005-4 is closed under the maintainer's replacement decision: malformed SHA/unknown tree mode errors, noncanonical path exclusions, no rejected-tree-entry list. FIND-8 and FIND-9 are distinct new root causes. The [R4 remediation task](TASK-005-R4-pr-file-size-and-root-policy.md) covers each sibling and routes to `$cr-implement`.

**Verification and limits:** `mise run gate` exited 0 at the candidate, including pipeline tests, symbolic journey, format, lints, review scripts and skill sync; `git diff --check base candidate` exited 0. Budget and durability reviewers ran their focused regression tests; task A ran `mise run test:pipeline`. The two blocking inputs are deterministic traced source paths without new executable regressions in this immutable candidate. No live GitHub/GHES or external storage adapter was exercised. Every required reviewer completed; no reviewer exceeded the 20-minute wait limit. The candidate remained `62346cb030354596cdf3dd7596de63dbd8d99fcb` throughout.
