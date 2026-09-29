# Co-change experiment — final change review

**Verdict: PASS.** Reviewed base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec` and immutable target `c3145c64a26ca98b699105668d54f8bdc861f9ec` against approved `SPEC-cochange-eval` revision 2. No material findings.

| Obligation | Implementation evidence at target | Verification evidence | Result |
|---|---|---|---|
| REQ-001 | `main.rs` discovers merged PRs once, samples 50 per repository, reads sampled base commit times, and shares complete history listings; `history.rs` bounds cold and incremental history | Fake-provider read-count and failure tests; TASK-001 round-4 PASS | PASS |
| REQ-002 | `history.rs` counts file-listing pages; `main.rs` separates production cold build, incremental update, zero review reads, and evaluation-only timestamp reads | Fake-provider counts and all three report `requests` objects | PASS |
| REQ-003 | `main.rs` and `report.rs` emit the fixed metadata, sample, trials, PR means, paired effects, decisions, costs, and validity threats | Three checked-in reports; independent shape and arithmetic checks; TASK-003 round-3 PASS | PASS |
| REQ-004 | `mise.toml` retains an explicitly invoked credentialed command outside `gate`; `main.rs` performs provider reads and prints only after the full run succeeds | Full diff and gate inspection; fake-provider failure test | PASS |
| INV-001, AC-004 | All executable changes are in the pipeline example; no production crate, public API, persisted format, grammar, extraction, packing, or gate change | Complete base-to-target diff and `mise run gate` | PASS |
| INV-002, AC-002 | Fixed repository cutoffs, trial rule, seed, ordering, and deterministic report construction | Task evidence records two byte-identical live runs; fake-provider determinism tests; report arithmetic check | PASS |
| INV-003 | `history.rs` uses strict merge-time-before-base-time filtering and excludes the trial PR; `trial.rs` excludes absent-base anchors and candidates | History boundary tests; TASK-002 round-2 PASS | PASS |
| INV-004 | Reports contain paths, IDs, ranks, scores, and counts; source is transient in memory; provider operations are reads | Source and JSON inspection; report-safety task review | PASS |
| INV-005 | Provider errors, incomplete listings, and exhausted rate limits fail before any report is emitted | Fake-provider failure tests and `run` source trace | PASS |
| AC-001 | `pydantic-ai.json`, `tokio.json`, and `zod.json` carry every REQ-003 field | Parsed report inspection: 50 PRs and 157, 143, and 165 trials respectively | PASS |
| AC-003 | `sample.rs`, `history.rs`, `trial.rs`, and `report.rs` cover selection, cold and incremental history, C/E/F ranking, PR means, bootstrap, and decision | No-network example tests in `mise run gate`; independent recomputation of per-PR and effect means | PASS |
| Constraints and non-goals | The experiment measures hit@10 ranking only at depth 10 and leaves production adoption, packing, other arms, tuning, and model exposure outside the diff | Spec and complete diff inspection; report field inspection | PASS |

**Task closure.** TASK-001 candidate `a2eeb67dfc4b739513c74dd085c0bd8ff549ca71`, TASK-002 candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`, and TASK-003 candidate `7ad38dbcb90af4e767c9d4b0165e3034ee9306ff` are ancestors of the target. Their final independent verdicts are PASS. The target adds only TASK-003's final review evidence after its reviewed candidate. Cross-task data flow from sample and history through trial and report was inspected at the target.

**Verification.** `mise run gate` and `git diff --check` passed at the target. A local read-only check parsed all three reports and recomputed sample counts, trial bounds, per-PR hit means, and E−C/F−C means. This review did not repeat credentialed live runs; TASK-003 evidence records two byte-identical runs and the independent task review checked the artifacts.

Completion payload: archive the ranking-only experiment with its fixed design, three reports, positive F−C decision-rule outcome, unchanged production behavior, and the live-run verification limit. Link the evaluator, reports, and current architecture authority.
