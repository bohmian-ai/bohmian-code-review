# TASK-001 task review — round 4

**Verdict: FIX_REQUIRED.** Immutable base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate `4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-001-pinned-sample.md`, and R1–R3 remediation tasks. The complete base-to-candidate diff was reviewed; HEAD remained pinned.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: named repositories and opt-in read-only command outside gate | `main.rs:37-42,256-305`; `mise.toml:76-78` | Diff and task graph inspection | PASS |
| REQ-002: deterministic 50-PR discovery, identity and chronology validation | `sample.rs:133-209`; `main.rs:206-248` | Eight focused sampler tests pass; R3 chronology path inspected | PASS |
| REQ-002–003: complete single file listing, source eligibility, base declarations and changed-line qualification | `sample.rs:241-279,314-379`; `main.rs:102-197` | Listing and screen tests pass | PASS |
| REQ-002: missing or malformed patches recorded as omissions | `Screen::new` at `sample.rs:345-355` | `FIND-TASK-001-7`: no-hunk patch is accepted but absent from omissions | **FAIL** |
| INV-001, AC-005 and non-goals: no production change, second listing, `PrContext`, replay, or trial blob fetch | Cumulative diff adds the private example, manifest entry, live task and change evidence | Complete diff inspection | PASS |
| INV-003: inconsistent provider metadata fails | `sample.rs:140-195,211-239,263-279` | Timestamp, chronology and duplicate tests pass | PASS |
| INV-004–005: source-free output, read-only in-memory sampling | `main.rs:120-157,270-295` | Source and output-path inspection | PASS |
| R1–R3 remediation obligations | `sample.rs:175-279,382-425`; `main.rs:61-249` | Focused tests and source inspection; `FIND-TASK-001-1` through `-6` closed | PASS |

## Independent reviews and validated finding

Wave 1 results: task review A **PASS**; task review B **FAIL**; standards **PASS**; sampling domain **FAIL**; source-trust domain **FAIL**. Citation verification exited 0. It rejected all citations for `DOMAIN-SAMPLING-1` and `DOMAIN-SAMPLING-2`, and one citation for `TASK-B-1`; those findings were dropped under the review contract. `SRC-R4-1` passed citation verification.

Wave 2 independently confirmed `SRC-R4-1` as **FIND-TASK-001-7** under the task-contract gate. `Screen::new` accepts `Some(Ok([]))` as a valid patch; empty and marker-only patch text can reach that result without a hunk, leaving `Screen::omitted` incomplete. A qualifying PR can therefore report an incomplete omission list. The root cause, reachable call path, reproduction, sibling inputs, and smallest correction are recorded in `findings-validation.md`. No notes remain. Prior `FIND-TASK-001-1` through `-6` are closed, and no earlier remediation became redundant. The self-contained correction task is `TASK-001-R4-no-hunk-patch-omission.md`; no spec revision is needed.

## Verification limits

Locally observed exit 0: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` (eight tests), `git diff --check e665bc17ce91e855e4a744f3231b58c1cd9cd0ab 4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`, and `mise run review:verify-citations changes/active/context-relevance-eval/review/TASK-001-r4 4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`. Candidate implementation evidence records owner tests, formatting, lints and gate passing; this review did not rerun those lanes. The confirmed gap was reproduced by a traced source path without editing the candidate. Credentialed live sampling and same-cutoff comparison belong to TASK-003.
