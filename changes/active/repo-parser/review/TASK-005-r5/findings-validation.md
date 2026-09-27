# TASK-005 R5 independent findings validation

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..866f777a1a41a03384c14c12276a5877dcac94a2`. Authority: approved `changes/active/repo-parser/spec.md` revision 13, original TASK-005, R1–R4 remediation, and both maintainer decisions. The decisions override conflicting remediation language; FIND-TASK-005-7 is withdrawn as invalid. I used the citation-checked claim ledger, not Wave 1 reasoning or verdicts, and inspected the cumulative candidate and its public facade paths independently.

## Validated ledger

**Empty.** Wave 1 supplied no surviving claims to merge or reproduce. Independent inspection found no blocking production, task-contract, repository-rule, source-trust, or durability gap in the reviewed TASK-005 path. No `FIND-*` ID is reopened or assigned. There are no nonblocking notes.

## Prior finding closure

| ID | Status and independent evidence |
|---|---|
| FIND-TASK-005-1 | Closed. Context publication creates a fresh key and propagates a failed create; it cannot report success by accepting an unverified pre-existing object (`context.rs:314-340`). The fresh key is derived from content digest and random bytes (`context.rs:434-451`). |
| FIND-TASK-005-2 | Closed. Each publication mints a separate context ID/key, while the store keeps that ID's original lifetime (`context.rs:314-340`, `testing.rs:428-451`). A later equal-content publication cannot extend an earlier ID. |
| FIND-TASK-005-3 | Closed. `near` passes all same-directory regular entries to shared classification before the scoped ingest allowance is spent; excluded paths do not consume indexed-file capacity (`pull_request.rs:589-599`, `snapshot.rs:787-825`, `pull_request.rs:222-242`). |
| FIND-TASK-005-4 | Closed under the R3 maintainer decision. The tree API fails the whole listing on unknown mode or malformed SHA and counts noncanonical paths as unsupported; it has no rejected-tree-entry list (`github.rs:957-993`, `github.rs:1387-1402`, `snapshot.rs:787-825`). Cold build and PR overlay consume that same validated listing (`snapshot.rs:577-607`, `pull_request.rs:155-204`). |
| FIND-TASK-005-5 | Closed. Canonical filenames from rejected PR records seed scoped discovery and join the changed-path set removed before secondary packing (`pull_request.rs:177-195`, `pull_request.rs:260-270`); rejection still appears in diagnostics and coverage (`pull_request.rs:211-219`, `pull_request.rs:248-259`). |
| FIND-TASK-005-6 | Closed. Every valid changed head is fetched for analysis, but a scoped head enters the index only while `ingest.indexed() < index_cap`; fallback gets only the remaining allowance (`pull_request.rs:222-242`, `pull_request.rs:395-442`). |
| FIND-TASK-005-7 | Withdrawn as invalid by `review/TASK-005-r4/maintainer-decision-clock-and-size.md`. The candidate follows the decision: `Contexts::collect_expired` asks the store for already-expired contexts and deletes object before record (`context.rs:400-426`); the store owns expiry decisions (`persistence.rs:258-292`). No older-than cleanup cutoff is required. |
| FIND-TASK-005-8 | Closed by shape. Changed-head and selected-candidate blob paths use `PrContext::fetch`, which gives `GitHubClient::blob` the invocation's `max_file_bytes`; the client reads at the smaller of that ceiling and its provider ceiling and returns no oversize bytes (`pull_request.rs:416-430`, `pull_request.rs:451-508`, `github.rs:1004-1032`). Cold/fallback ingest and root policy fetch use the same bounded blob API (`snapshot.rs:750-768`, `snapshot.rs:959-987`); archive ingest has its file bound. The snapshot scope and key contain `max_file_bytes`, excluding looser-limit facts from reuse (`snapshot.rs:337-358`, `storage.rs:68-83`). No post-fetch size guard is prescribed. |
| FIND-TASK-005-9 | Closed. The shared root `.gitattributes` loader accepts both `File` and `Executable` and is used by cold/update and PR overlay/scoped paths (`snapshot.rs:750-768`, `snapshot.rs:577-607`, `pull_request.rs:165-174`). |

## Verification and limit

`mise run test:pipeline` exited 0 on this candidate; `git diff --check fa4fa2101950626a9fe60b0fdff0439b04c16f01 866f777a1a41a03384c14c12276a5877dcac94a2` exited 0. I inspected the targeted regression tests for scoped capacity, PR file limits, rejected changed paths, executable root policy, expiry and interrupted publication. I did not rerun the full gate, independently exercise a live provider, or edit candidate code. The orchestrator should report any missing required review or broader verification in `verdict.md`.

**Recommendation: PASS**, subject to the orchestrator confirming complete review artifacts and required verification. The validated blocking ledger is empty.
