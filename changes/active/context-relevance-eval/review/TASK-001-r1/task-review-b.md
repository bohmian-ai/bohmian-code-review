# TASK-001 independent implementation review B

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..c626450649edfdbea105011ab5293267df49ed74`; approved spec revision 4. Reviewed the complete five-file diff, task, repository rules, sampler, and its pipeline dependencies. Focused check: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` — 4 passed.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001: both named GitHub repositories | `main.rs` `REPOSITORIES`, `Api`, `GitHubClient` | Source inspection; live run belongs to TASK-003 | PASS |
| REQ-002: 50 qualifying merged default-branch PRs before cutoff; deterministic order; one complete listing; specified failures and patch omissions | `select`, `Discovery`, `listing`, `Screen`, `base_snapshot` | Four sampler tests pass; timestamp validation gap below | FAIL |
| REQ-003: modified path with base declarations is withholdable | `Screen::qualifies`, `qualify` snapshot lookup | `screen_qualifies_and_records_omissions` | PASS |
| REQ-007: credentialed, read-only opt-in invocation outside gate | `mise.toml` `live:context-relevance`; example only reads GitHub | Source and task graph inspection | PASS |
| INV-001 and AC-005: production unchanged; one PR listing; no `PrContext` or replay | Diff changes example, manifest, task and live lane only; `listing` called once in `qualify` | Complete diff inspection | PASS |
| INV-003: reproducible sample and failures on inconsistent input | Merge-time ordering and duplicate detection in `Discovery`; invalid dates pass `is_timestamp` | Focused tests do not cover calendar-invalid dates | FAIL |
| INV-004: source-free report | JSON output contains metadata, paths and omission reasons | Source inspection | PASS |
| INV-005: read-only, no checkout, run-local source | GitHub reads and memory snapshot adapters | Source inspection | PASS |
| Task non-goals: no trial blobs, second listing, production API, persisted format, or gate change | Complete diff | Source inspection | PASS |

## Proposed finding

**TASK-B-1 — INCORRECT; timestamp shape is accepted as a real timestamp.** Root cause and defect class: `sample.rs` `is_timestamp` checks only digit positions and separators, so invalid calendar and clock values are treated as usable. Its callers are `Discovery::new` (cutoff) and `Discovery::offer` (PR update and merge times); all three trust the same weak validation. For example, `2026-13-45T99:99:99Z` passes the predicate. A listed PR with that merge time can enter `pending`, sort ahead of real PRs, and consume one of the 50 sample slots. This violates REQ-002's requirement that an unusable merge timestamp fail the run and INV-003's reproducibility boundary. The function is the owner; the `String` wire fields are acceptable if this boundary validates them. **Correction:** validate calendar date and time, including month length and leap years, for cutoff, update and merge values before comparison; add a focused invalid-date test for each path. No additional provider call or dependency is needed.

Overall: **FAIL**.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":203,"snippet":"    text.len() == SHAPE.len()","symbols":["is_timestamp","Discovery::new","Discovery::offer"]}]
```
