# TASK-001 round 3 — independent task review B

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..c88bea58b62df3854b28a082fc7dd969ed8cfd6a`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-001-pinned-sample.md`, and R1/R2 remediation tasks. I inspected the cumulative diff and the command-to-provider and snapshot paths. The exact focused sampler command passed seven tests; `git diff --check` passed. The credentialed live sample and same-cutoff rerun remain TASK-003 obligations.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: named repositories and an opt-in credentialed read-only command | `main.rs:37-42,256-296`; `mise.toml` live lane | Source and task graph inspection | PASS |
| REQ-002: select 50 merged default-branch PRs before a fixed cutoff, ordered by merge time then number, or fail clearly on inconsistent input | `sample.rs:140-201`; `main.rs:206-248` | Ordering test passes, but the chronology path in TASK-B-R3-1 remains reachable | FAIL |
| REQ-002, INV-003: one complete PR listing; rejected and repeated paths or incomplete pagination fail | `main.rs:102-118`; `sample.rs:235-272,308-354` | Listing cap, duplicate-path and rejected-entry tests pass | PASS |
| REQ-002–003: 2–15 eligible files, one modified file with base declarations, another with changed lines; unusable patches are omissions | `main.rs:165-197`; `sample.rs:308-373` | Qualification and omission test passes | PASS |
| R1 findings: provider file ceiling and test-item rustdoc | `sample.rs:249-272,376-419` | Cap test passes; source inspection | PASS |
| R2 findings: repeated paths, real Gregorian timestamps, concrete workflow owner | `sample.rs:205-272`; `main.rs:61-249` | Duplicate and timestamp tests pass; source inspection | PASS |
| INV-001, AC-005 and task non-goals: no production change, second listing, `PrContext`, replay, trial blobs, format or gate change | Diff adds the private example, its manifest entry, live lane and task evidence | Complete base-to-candidate diff inspection | PASS |
| INV-004–005: source-free output and in-memory, read-only source use | `main.rs:120-157,251-296` | Source inspection | PASS |

## TASK-B-R3-1 — inconsistent merge/update chronology reaches selection

- **Classification:** INCORRECT. **Defect class:** paired provider timestamps are validated individually but their required chronological relation is unchecked. This is distinct from R2's invalid-calendar root cause; both timestamps here can be valid UTC instants.
- **Root cause and location:** `Discovery::offer` in `sample.rs:169-187` validates `updated_at` and `merged_at` separately, then enqueues a merged PR even when `merged_at > updated_at`. The owner's `ListedPr` fields remain raw strings, which permits the inconsistent pair. The sibling consequence is `Discovery::settled` at `sample.rs:194-201`, which assumes merge time never exceeds update time; `Api::select` at `main.rs:219-239` takes the minimum update time on a page as a proof that settled PRs outrank all unread PRs. I found no other timestamp producer or ordering sink in the sampler.
- **Violated obligation:** REQ-002 requires the 50 most recently merged PRs in final merge-time order; INV-003 says provider inconsistency fails the run. The module's own invariant at `sample.rs:3-5` states the relation on which early settlement relies.
- **Evidence and observable consequence:** `Discovery::offer` accepts an entry updated `2026-01-01T00:00:00Z` but merged `2026-01-02T00:00:00Z`, provided the cutoff is later. Both values pass `is_timestamp`; no comparison rejects their impossible relation. This can settle a PR on the wrong side of a page boundary; a later inconsistent entry can then outrank an already emitted PR. Even without that second entry, the run accepts provider input which contradicts the ordering proof, instead of failing as INV-003 requires.
- **Testable correction:** At `Discovery::offer`, reject a merged PR whose merge time is after its update time through the existing `SampleError::UnusablePr` path before adding it to `pending`. A focused in-memory test should show that valid UTC values in that reversed relation fail, while equal and earlier merge times preserve ordering. This requires no new parser, provider request, or public API.

Overall **FAIL**. No further task finding arose from the R1/R2 corrections. This review does not ask for the live TASK-003 proof in TASK-001.

```citations
[{"id":"TASK-B-R3-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":178,"snippet":"        if listed.base.name != self.branch || merged_at >= self.cutoff {","symbols":["offer"]},{"id":"TASK-B-R3-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":198,"snippet":"            .partition(|pr| read_until.is_none_or(|until| pr.merged_at.as_str() > until));","symbols":["settled"]},{"id":"TASK-B-R3-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":227,"snippet":"            let read_until = listed.iter().map(|pr| pr.updated_at.clone()).min();","symbols":["select"]}]
```
