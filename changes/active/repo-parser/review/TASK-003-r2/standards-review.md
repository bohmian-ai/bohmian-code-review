# TASK-003 r2 repository standards review

Subject: base `60cc56e`, candidate `a10c874ff346d063714be75b46b6ec2c34b990b5`, approved `SPEC-repo-parser@11`; complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs` plus the base-to-candidate diff. No other reviewer report was used. The candidate remained at the named commit during inspection. `.codegraph/` is absent.

## Authority coverage

| Changed surface | Applicable authority inspected |
|---|---|
| Pipeline crate ownership, public exports, and dependencies (`lib.rs`, `Cargo.toml`, `Cargo.lock`) | `AGENTS.md:13-34`, `architecture/agent-rules.md:3-10`, `architecture/bohmian-code-review-design.md` crate direction, `architecture/references/languages/rust-core.md` |
| Eligibility and root attributes (`eligibility.rs`) | `AGENTS.md:23,25-34`, `architecture/agent-rules.md:5,10`, design public behavior, Rust core; approved spec revision 11 source and eligibility boundaries |
| Archive ingest, snapshot coverage, serialization, publication, read-back, cleanup (`snapshot.rs`) | `AGENTS.md:21-34`, `architecture/agent-rules.md:3-10`, design crate direction/public behavior, Rust core; approved spec revision 11 REQ-005/006/015/016 and INV-002/003/005/007 |
| Injected snapshot persistence and storage (`persistence.rs`, `storage.rs`) | `AGENTS.md:23,25-34`, `architecture/agent-rules.md:3,7-10`, design crate direction, Rust core; approved spec revision 11 REQ-006/015/016 |
| Inline owner tests and shared test doubles (`snapshot.rs`, `eligibility.rs`, `testing.rs`) | `AGENTS.md:32,36-40`, `architecture/agent-rules.md:4,9`, `architecture/references/languages/testing-workflows.md:3-20`, `architecture/references/languages/implementation-execution.md:207-232`, spec-driven development review/test discipline; `mise.toml` scoped and gate tasks |
| Task and prior remediation evidence added within the diff | `AGENTS.md:42-46`, `architecture/agent-rules.md:13-14`, `architecture/references/languages/spec-driven-development.md` lifecycle/review sections |

## Rule results

| Written rule | Source evidence and result |
|---|---|
| Pipeline is sole Git/storage IO owner and consumes the symbolic facade only (`AGENTS.md:21-23`; design crate direction) | **PASS.** Changed production files stay in `bohmian-code-review-pipeline`; `snapshot.rs:16-33` uses the public `bohmian_code_review` facade, and the crate manifest adds no dependency. No symbolic crate changed in this diff. |
| Concrete owners for stateful workflows; no duplicate symbolic stage (`AGENTS.md:27`; agent rules:3,7) | **PASS.** `ColdBuild`, `Ingest`, and `Snapshots` own their respective workflows (`snapshot.rs:227-313,398-836`); remaining free helpers are stateless. The candidate removes public IO free functions. |
| Transient source, source-free facts, validated paths and exact object identity (`AGENTS.md:21-23,29`; agent rules:5-6; design public behavior) | **PASS for repository-rule shape.** `Ingest::stream_entries` reads one entry into a local byte vector and `Snapshot` serializes paths, blob IDs, facts, and coverage (`snapshot.rs:153-168,696-738,539-566`). `Snapshots::read` checks the identity-derived key and digest before decode (`snapshot.rs:252-276`). Detailed trust behavior remains for domain review. |
| Error handling, imports, rustdoc, grammar/dependency integrity (`AGENTS.md:30-34`; Rust core) | **PASS.** Changed fallible Rust items have `# Errors`; signatures use bare imported types; no changed production `unwrap`, lint suppression, grammar pin, generated output, or added dependency was found. `Cargo.lock` is unchanged. |
| Tests inline with owner, credential-free; required scoped/journey/gate lanes (`AGENTS.md:36-40`; agent rules:4,9; testing workflows:3-20) | **PASS for placement and configured lanes.** New snapshot and eligibility tests are inline, shared fakes are `cfg(test)`, and `mise.toml` has `test:pipeline`, `test:symbolic-context:journey`, `fmt:check`, `lints`, and `gate`. Task evidence records exit 0 for these lanes; this review did not rerun Cargo because the mechanically enforced rules are covered by those results. |
| Do not add sleeps to pass a check in place of deterministic synchronization (`architecture/references/languages/implementation-execution.md:224-229`) | **FAIL.** New deadline test at `snapshot.rs:1791-1799` loops until wall-clock expiry with a 5 ms sleep. Its success depends on a one-second real-time wait rather than controlled deadline state. |
| Review subject, spec authority, and Git boundaries (`AGENTS.md:42-46`; agent rules:13-14; spec-driven development review section) | **PASS.** The task records approved revision 11, the cumulative candidate is the named commit, and the diff contains no publication or identity changes. |

## Finding

### STD-R2-1 — Deadline test uses a wall-clock sleep

- **Classification:** VIOLATION of a written test integrity rule.
- **Root cause / defect class:** the new test induces deadline expiry by waiting for elapsed real time, instead of using deterministic control of the execution clock or another controlled expiry seam. This is a test-proof defect; the production `Execution` API shape does not itself establish a repository-rule violation here.
- **Location and sibling sweep:** `crates/bohmian-code-review-pipeline/src/snapshot.rs:1791-1799`; the `interruption_during_a_complete_archive_never_publishes` test contains the only added `std::thread::sleep` in the candidate production/test surfaces inspected. Its cancellation half uses a deterministic transport hook (`snapshot.rs:1782-1789`).
- **Written rule:** `architecture/references/languages/implementation-execution.md:224-229` says never pass a check by adding sleeps instead of deterministic synchronization.
- **Evidence / consequence:** the test blocks in the archive request hook until `Instant::now()` passes a deadline set one second ahead, sleeping every 5 ms. It adds at least a second of real-time latency and makes deadline proof dependent on wall time and scheduling.
- **Testable correction:** exercise the same deadline-during-archive path with controlled time or a deterministic expiry trigger, preserving the assertions that the result is `DeadlineExceeded`, no archive entry was indexed, no object was written, and the prior pointer stayed put. Do not delete deadline coverage or weaken its assertions.

Overall: **FAIL** (one written-rule finding).

```citations
[{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1797,"snippet":"                    std::thread::sleep(Duration::from_millis(5));","symbols":["interruption_during_a_complete_archive_never_publishes"]}]
```
