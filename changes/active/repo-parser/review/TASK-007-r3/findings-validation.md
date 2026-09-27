# TASK-007 r3 findings validation

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..b721917420113ab2e3530966e9e456eecc89dcff`; approved `SPEC-repo-parser@14`; original TASK-007; R1 and R2 remediation; stateless-snapshot maintainer decision. Wave 1 claim input: `claim-ledger.md`. Citation check exited 0. Recommendation: **FIX_REQUIRED**.

## Blocking ledger

### FIND-TASK-007-3 — Changed Rust test items lack their own accurate rustdoc

- **Source/status:** `STD-R3-1`; **CONFIRMED**, new ID. The two cited sites share the documentation defect class and written rule; neither reopens the behavior defects `FIND-TASK-007-1` or `FIND-TASK-007-2`.
- **Gate/classification:** Repository standard; **VIOLATION** of `AGENTS.md:32` and `architecture/agent-rules.md:11`, which require meaningful rustdoc on new or materially changed Rust items, including tests.
- **Root cause/defect class:** The R1 insertion in `snapshot.rs` put `fixed_commit_build_ignores_head_advance_and_run_follows_it` between the original Scenario 1 doc and its owner, `complete_cold_snapshot_is_verified_source_free_and_reloadable`. The new test inherited a paragraph describing the original cold journey, while that original test now has no rustdoc. The new `provider_contract.rs` test module was added without module rustdoc. This is inaccurate or absent item documentation at the insertion sites, not a behavioral failure.
- **Siblings:** `crates/bohmian-code-review-pipeline/src/snapshot.rs:1703-1711,1743-1744`; `crates/bohmian-code-review-pipeline/examples/provider_contract.rs:327-328`. The fixed-commit test and original cold journey are one documentation-placement site; the new module is the other. Other newly introduced provider constants, functions, and tests carry rustdoc.
- **Reproduction/evidence:** At base `2257992`, the five-line Scenario 1 comment immediately precedes `complete_cold_snapshot_is_verified_source_free_and_reloadable` (`git show 2257992:crates/bohmian-code-review-pipeline/src/snapshot.rs`, near line 1674). At candidate `b721917`, the same comment begins at `snapshot.rs:1703`, adds a fixed-commit sentence at `:1708-1709`, and immediately precedes `fixed_commit_build_ignores_head_advance_and_run_follows_it` at `:1711`; `complete_cold_snapshot_is_verified_source_free_and_reloadable` begins at `:1744` with only `#[tokio::test]` above it. `provider_contract.rs:327-328` has `#[cfg(test)]` then `mod tests` without a preceding `///` or `/**` comment. These are direct source-order violations of the cited rule.
- **Consequence:** The Scenario 1 cold journey is undocumented, and the fixed-commit test carries a misleading cold-journey description; the new provider test module has no stated purpose or boundary. Runtime and test behavior are unaffected.
- **Smallest safe correction:** Restore the Scenario 1 rustdoc immediately above its original test, give the fixed-commit test its own accurate rustdoc, and add one useful module rustdoc before `mod tests` describing the captured-report contract validation. Keep all test bodies and provider behavior intact. No helper, dependency, or harness change is warranted.
- **Focused closure proof:** Inspect the three item boundaries after the edit: each has meaningful rustdoc directly attached to the item it describes. Run `mise run fmt:check`, `mise run lints`, and the focused pipeline test lane required by the remediation packet. No new behavioral test is needed for a comment-only correction.

## Prior finding closure and remediation drift

| Prior finding | Closure evidence | Result |
|---|---|---|
| `FIND-TASK-007-1` | `live_pr.rs:122-140` reads one PR and sends first and warm samples to `PrContext::run_at`; `pull_request.rs:173-183,379-419` lists under that read and rejects a changed base/head before publication. `samples_at_one_read_pr_refuse_a_moved_head` covers stable and moved inputs. TASK-007 rows and reports now state fixed cold commit/tree and PR base/head. | **CLOSED** |
| `FIND-TASK-007-2` | `provider_contract.rs:216-232` fails the command on `failures(&cases)`; `:256-307` checks required cases, tree usability, archive shape, patch omission, conditional 304, and API rate headers. `every_named_case_decides_the_verdict` breaks each class and asserts rejection. | **CLOSED** |

R2 did not leave an identified redundant guard, parallel implementation, or persistence change. `PrContext::run` keeps fresh PR pinning; the opt-in benchmark uses the candidate-owned `run_at` path. The stateless snapshot decision remains intact.

## Claim disposition and limits

| Source claim | Status | Gate |
|---|---|
| `STD-R3-1` | **CONFIRMED** → `FIND-TASK-007-3` | Written repository rustdoc rule |

No rejected claims or notes. Validation used the immutable source, base-to-candidate diff, written rule, prior ledger, and focused caller/source tracing. It did not rerun credentialed GitHub or capacity measurements; their captured reports and prior implementation evidence remain the verification basis. The documentation finding is reproducible from source order and needs no runtime probe.
