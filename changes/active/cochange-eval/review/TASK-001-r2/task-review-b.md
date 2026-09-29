# TASK-001 task review B

Base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`; candidate `20fa48d5cfecf0d4970d325d7334e0aede671728`. Authority: approved `changes/active/cochange-eval/spec.md` revision 2, original TASK-001, and TASK-001-R1. I inspected the cumulative diff and source, not the implementation evidence tables. `mise run test:pipeline` passed independently (23 context-relevance example tests).

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Three fixed cutoffs; deterministic 50-PR sample; exact source eligibility | `main.rs:65-85,402-450`; `sample.rs:140-259,334-396` | Repository, discovery, and TypeScript screening tests; fake run selects 50 | PASS |
| Base committer timestamps; earliest cold point; depth 10; incremental time boundary; exclude trial PR and mass edits | `main.rs:254-279,451-465`; `history.rs:33-49,88-147` | History unit tests and fake run with a complete 31-file, two-page history listing | PASS |
| One shared merged-PR listing; complete, bounded PR-file reads; no per-trial history fetch | `main.rs:190-213,393-473`; `history.rs:99-139` | Fake provider asserts ordered direct and listing reads; missing, incomplete, and exhausted rate-limit cases fail | PASS |
| Source-free report emitted only after every repository succeeds | `main.rs:495-592` | Fake `run` returns errors without records; output remains after `run` | PASS |
| Production provider-request costs, including cold build and each incremental update, and separate evaluation-only timestamp reads | `history.rs:62-70,119-124`; `main.rs:546-563` | Fake counts a two-page history listing, but expected aggregate cost counts that listing as one | FAIL (`TASK-B-1`) |
| Production crates, public APIs, persisted formats, grammar pins, gate, historical reports, and ranking arms unchanged | Base-to-candidate diff touches example, opt-in mise description, and change packet only | Diff inspection; pipeline lane passed | PASS |
| Prior R1 findings: incomplete mass listings, real provider proof, per-PR unit, Zod suffix set | `history.rs:33-49`; `main.rs:149-157,458-465,555-558`; `sample.rs:243-259` | Focused fake run and TypeScript tests | PASS for the four diagnosed behaviors; request-page accounting is a new defect |

## Finding

**TASK-B-1 — INCORRECT, provider-request cost counts logical PR listings instead of HTTP pages.** Defect class: request units are lost at the paginated listing owner. `Api::listing` can issue multiple `pull_request_files` calls for one PR, but returns only `Listing`; `History::build` then increments `reads.cold` or `reads.incremental` once per PR. `record` labels those counts as production request costs. This affects both cold and incremental history listings. The fake's PR 15 has two pages, yet the report asserts `aggregate_through_newest_base: 49`, the number of incremental PR listings; actual incremental PR-file requests are 50. A cold-history PR with two pages similarly makes `cold_build` too small. REQ-002 calls these figures provider requests, and TASK-001 Scenario 3 requires accurate cost categories. The observable report understates real cost for any paginated history PR. Keep the per-merged-PR logical update unit if useful, but count actual provider calls at the shared `Api::listing` owner, propagate the count through cached qualification and history-only reads, and report actual cold and aggregate request counts. Focused proof should place a multi-page PR in each phase and compare the cost fields with fake transport request counts. The owning shape currently discards pagination count, permitting the error.

No other blocking finding emerged. No live credentialed run was performed; the fake path and repository lane were the available verification. Overall result: **FAIL**.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":204,"snippet":"            let read = self","symbols":["listing"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":121,"snippet":"                history.reads.cold += 1;","symbols":["History::build"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":123,"snippet":"                history.reads.incremental += 1;","symbols":["History::build"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":554,"snippet":"                \"cold_build\": requests.listing_pages + requests.history.cold,","symbols":["record"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":557,"snippet":"                    \"aggregate_through_newest_base\": requests.history.incremental,","symbols":["record"]}]
```
