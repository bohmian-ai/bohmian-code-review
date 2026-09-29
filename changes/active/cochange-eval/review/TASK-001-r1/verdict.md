# TASK-001 review verdict: FIX_REQUIRED

## Immutable subject

- Base: `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`
- Candidate: `f880e9e5b2621166f5240f179f2393f836e259ea`
- Authority: `changes/active/cochange-eval/spec.md` revision 2 and `tasks/TASK-001-history.md`
- Scope: the complete five-file base-to-candidate diff. HEAD remained the candidate throughout review.

## Acceptance matrix

| Obligation | Evidence | Result |
|---|---|---|
| Three fixed repositories and deterministic 50-PR selection | `main.rs` `REPOSITORIES` and `Api::select`; sampler tests | PASS for ordering and cutoffs; FAIL for Zod suffix eligibility (`FIND-TASK-001-4`) |
| Base commit times, cold depth ten, time eligibility, and trial-PR exclusion | `Api::base_time`, `History::build`, `History::for_trial`; focused history tests | PASS |
| Complete bounded provider reads and mass-edit exclusion | `Api::listing`, `history::changed`; `history_fails_on_short_or_incomplete_input` | FAIL: incomplete/rejected mass edits can succeed (`FIND-TASK-001-1`) |
| One shared listing path, no per-trial history reads, accurate cost categories, and complete failure | `Api::select`, `run`, `record`; pure history counting test | FAIL: required fake-provider orchestration proof absent (`FIND-TASK-001-2`); incremental cost has wrong unit (`FIND-TASK-001-3`) |
| Opt-in, credentialed, read-only command; no production or gate change | `mise.toml`, example-only diff, buffered report output | PASS |
| No ranking-arm, packing, production adoption, or historical-report changes | Complete diff | PASS |

## Review results

Wave 1: task review A **FAIL**; task review B **FAIL**; standards review **PASS**; history/provider domain review **FAIL**. The citation check passed for every final Wave 1 citation; `citations.txt` has no rejected citation. Wave 2 independently confirmed four distinct blocking root causes in `findings-validation.md`:

| ID | Status | Gate | Root cause |
|---|---|---|---|
| `FIND-TASK-001-1` | CONFIRMED | REQ-001, INV-002 | Mass-edit classification precedes provider-listing completeness validation. |
| `FIND-TASK-001-2` | CONFIRMED | AC-003, task acceptance | The actual provider orchestration has no no-network fake-provider proof. |
| `FIND-TASK-001-3` | CONFIRMED | REQ-002 | A cumulative history read count is labeled as one production incremental update. |
| `FIND-TASK-001-4` | CONFIRMED | Spec Sample, task Scenario 1 | Language-based Zod eligibility also admits `.mts` and `.cts`. |

There are no notes, rejected claims, or prior findings to close. See `findings-validation.md` for each reproduction, sibling sweep, consequence, and correction boundary. The single remediation task is `TASK-001-R1-history-contract.md`.

## Verification limits

Independent local checks passed: `mise run test:pipeline` (21 context-relevance example tests), `mise run fmt:check`, `mise run lints`, and `git diff --check a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec f880e9e`. The candidate task records a passing `mise run gate`; this review did not rerun it. No live credentialed run was made. Current tests do not exercise `Api::select` or `run` with a fake provider, which is itself blocking finding `FIND-TASK-001-2`.

**Verdict: FIX_REQUIRED.**
