# TASK-003 r4 verdict — FIX_REQUIRED

## Immutable subject

Base `c74871cbc78140820c9b0a5fda60e0952c4265e4`; candidate `0a1dea22b206e6174d4421d5ba7f7590ded93673` (HEAD remained at this commit throughout review). Authority: [approved revision 12](../../spec.md), [TASK-003](../../tasks/TASK-003-cold-snapshot-and-scoped-context.md), `AGENTS.md`, and applicable architecture references. The complete base-to-candidate diff was reviewed. Earlier r1–r3 findings concern a superseded synchronous design and were rechecked only where their defect class still applies.

## Acceptance matrix

| Obligation | Candidate evidence | Verification | Result |
|---|---|---|---|
| Stream a verified, bounded cold archive without retaining the source set | `archive.rs` `TarStream`; `snapshot.rs` archive ingest | Source-path reproduction in `findings-validation.md` | **FAIL — FIND-TASK-003-8** |
| Complete source-free snapshot, deterministic serialization, warm reload | `ColdBuild::run`, snapshot serialization and read | Owner tests and full `mise run gate` | PASS |
| Incomplete build reports coverage and preserves pointer, including archive-bound exhaustion | `ColdBuild::run`, `IngestStats`, `Coverage` | Existing expansion-bound test currently expects publication | **FAIL — FIND-TASK-003-11** |
| Provider and file limits do not mislabel unverified eligible files | `GitHubClient::blob`, `Ingest::fallback` | Source-path reproduction with unequal limits | **FAIL — FIND-TASK-003-10** |
| Record → object → pointer publication; drop-safe retry and duplicate validation | `ColdBuild::publish` | Paused-clock drop, adoption, and CAS tests; gate | PASS sequentially; concurrent collection fails below |
| Collect only stale, unreferenced snapshots; preserve re-touched and pointed-to objects | `Snapshots::collect`, `SnapshotStore`, in-memory store | Concurrent source-path interleaving; serial owner test lacks this schedule | **FAIL — FIND-TASK-003-9** |
| Separate injected snapshot store, tenant/installation isolation, repository removal | `persistence.rs`, `testing.rs`, `Snapshots` | Owner tests; standards review | PASS subject to collection race |
| Root `.gitattributes` reset semantics | `eligibility.rs` tri-state handling | `attribute_resets_read_as_unset`; gate | PASS |
| No detached spawn, backend, scoped PR indexing, incremental update, source persistence, or library deadline | Complete diff | Source inspection; `rg 'tokio::spawn'` empty; gate | PASS |

## Review results

Wave 1: task reviews A and B **FAIL**; standards review **PASS**; source-trust, concurrency, and resource-budget domain reviews **FAIL**. The citation check in `citations.txt` returned zero rejected entries. The fresh Wave 2 review independently reproduced and merged the claims in `findings-validation.md`.

| Finding | Wave 1 sources | Validated root cause | Gate |
|---|---|---|---|
| **FIND-TASK-003-8** | TASK-A-1, DOM-TRUST-R4-1, BUDGET-1 | Verified file buffers accumulate until the whole compressed chunk is decoded | REQ-005/013, Scenario 1 |
| **FIND-TASK-003-9** | TASK-A-2, TASK-B-1, CONC-1 | A stale cleanup list does not protect against a concurrent record refresh and pointer publication | REQ-006/013, Scenario 4; durability |
| **FIND-TASK-003-10** | BUDGET-2 | Provider blob ceiling is classified as verified file oversize | REQ-005, INV-002/003; source trust |
| **FIND-TASK-003-11** | BUDGET-3 | Archive-bound exhaustion is diagnostic only and does not prevent publication | REQ-005/013, Scenario 2 |

No claim was rejected or reduced to a note. Prior FIND-1 through FIND-7 are not reopened: the synchronous cancellation/deadline obligations were superseded; record-before-object, coverage, standards, and attribute-reset classes were checked against this candidate. The collection race was previously only a note, but revision 12's explicit collection and durable-publication contract makes this candidate's race blocking.

The hand-written `TarStream` is warranted here. The already-installed `tar` crate pulls from synchronous `Read`; the provider yields async chunks, and whole-archive buffering or a detached blocking worker conflicts with the approved bounds and cancellation model. The parser's verified-file handoff still needs FIND-8's bound.

## Verification and limit

`mise run gate`, `git diff --check`, and the no-`tokio::spawn` search passed. The complete gate includes `fmt:check`, `lints`, pipeline tests, the public symbolic journey, and review-script checks. The four failures are source-path reproductions, including an existing test that explicitly expects the wrong archive-bound result. No production source was changed during review; closure tests remain implementation work. All required reviewers completed within the review window.

**Verdict: FIX_REQUIRED.** [Remediation task](TASK-003-R4-cold-ingest-and-collection.md): FIND-TASK-003-8, FIND-TASK-003-9, FIND-TASK-003-10, FIND-TASK-003-11.
