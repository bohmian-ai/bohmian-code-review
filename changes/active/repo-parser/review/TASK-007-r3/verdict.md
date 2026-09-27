# TASK-007 task review, round 3

**Verdict: FIX_REQUIRED.** Blocking finding: `FIND-TASK-007-3`. Remediation: [TASK-007-R3-test-rustdoc-ownership.md](TASK-007-R3-test-rustdoc-ownership.md).

## Immutable subject

- Base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `b721917420113ab2e3530966e9e456eecc89dcff`, unchanged throughout review.
- Approved `SPEC-repo-parser@14`; original TASK-007; R1 and R2 remediation; prior r1/r2 verdicts and ledgers; `review/maintainer-decision-stateless-snapshot.md`; `AGENTS.md` and applicable architecture rules and references.
- The full cumulative diff includes the fixed-commit and stable-PR measurement paths, controlled provider command and validation, focused tests, refreshed real-provider and capacity reports, and prior review evidence.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independent facades; composed cold, update, PR and read journey; scoped fallback and retry behavior; S1–S19 and oracles | Existing public journey and stage owners; `ColdBuild::run_at` and `PrContext::run_at` reuse the existing build and publication paths | `mise run gate` passed locally; journey and focused tests also passed in Wave 1 | PASS |
| Configurable S20 against a real existing PR, nonempty context, read-back and visible failures; AC-011 | `examples/live_pr.rs` and checked-in Wyrd reports | PR #1 records 50 spans, full coverage and equal read-back; PR #91 records an explicit empty-context failure | PASS |
| Fixed 1k/10k/50k capacity workloads and complete snapshots after TASK-008/009/010; AC-007; prior `FIND-TASK-007-1` | Fixed cold commit/tree, one PR base/head per run, movement rejection; Wyrd, VS Code and Rust reports | Focused fixed-commit and moved-PR tests passed; three reports reconcile eligible counts and record three warm samples, calls, timings, snapshot bytes and coverage | PASS — prior finding closed |
| Controlled opt-in provider contract; AC-006; prior `FIND-TASK-007-2` | Production client/transport and `provider_contract.rs::failures`; recorded seven-case report | Exact provider-verdict test passed; captured live report records passing named cases | PASS — prior finding closed |
| Stateless, source-free snapshots; no consumer, server, worker, clone, durable source archive or credentialed normal gate lane | Cumulative diff, snapshot payload, manifests and `mise.toml` | Source and diff inspection; `git diff --check 2257992 b721917` passed | PASS |
| Useful rustdoc on new/materially changed Rust items (`AGENTS.md:32`; `architecture/agent-rules.md:11`) | `snapshot.rs:1703-1744`; `provider_contract.rs:327-328` | Direct candidate/base source comparison and citation check | **FAIL — `FIND-TASK-007-3`** |

## Review disposition

Wave 1: task A **PASS**, task B **PASS**, capacity domain **PASS**, provider domain **PASS**, standards **FAIL** (`STD-R3-1`). The citation check exited 0 with all three cited locations `OK`; no citations were rejected. Wave 2 independently reproduced the written-rule violation and confirmed it as `FIND-TASK-007-3`. There are no notes or rejected claims. Prior `FIND-TASK-007-1` and `FIND-TASK-007-2` are **CLOSED**; no redundant remediation code was identified.

## Verification limits

The local `mise run gate` and cumulative `git diff --check` exited 0. Reviewers ran the focused fixed-commit, moved-PR, provider-verdict and public-journey tests. This review inspected the checked-in credentialed GitHub and capacity evidence without repeating live calls or measuring RSS. Live 403/429 and GHES behavior remains supported by local fake-transport tests. These limits do not affect reproduction of the rustdoc finding, which follows directly from source order and the written rule.
