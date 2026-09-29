# Ranker pipeline R4 task review A

**Result: PASS.** Immutable subject: base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `cd93bdadfa5cb17ed54752168e4a445c2e660068`. Authority: `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and R1–R3 remediation tasks. The maintainer directed that there be no original task file. I reviewed the cumulative diff and traced the R3 boundaries through their consumers. The untracked `changes/active/impact-map/` is outside the subject and was untouched.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free Ranker; S/C/E/F semantics | `select/src/rank.rs`; facade re-exports and pipeline default | Ranker and public symbolic journey tests in recorded R3 gate | PASS |
| 2. Fused default; empty history follows rarity order | `DEFAULT_RANKER`, `Fused::rank` | Selector fusion tests and public journey in recorded R3 gate | PASS |
| 3. Feature-gated reusable named-ranker evaluation; fixed report compatibility | `eval/src/{trial,report,live}.rs`; R2 pair-derived report keys/rule | Named-arm fake-provider test and checked-in report regression in recorded R3 gate | PASS |
| 4. Source-free distinct persisted reference names, snapshot v4 | `core/src/facts.rs` validates names; index load validates before replacement; snapshot decode validates persisted facts | Focused core and snapshot tests passed in this review; facade round-trip and index tests in recorded R3 gate | PASS |
| 5. Fractional scores; context v2 | `SelectedSpan.score: f64`; `context.rs` version | Selector and pipeline tests in recorded R3 gate | PASS |
| 6. Co-change-only candidates pack indexed declarations within budgets | Selector candidate fallback | Selector and symbolic journey tests in recorded R3 gate | PASS |
| 7. Complete-or-fail bounded history in both passes; time eligibility; no partial publication | `CoChangeHistory::read` now fails at unjoined page bound regardless of new-PR count; update and cold build consume this result before publication; PR context filters by base time and PR number | Focused ten-new-PR cap test passed in this review; update/cold publication and journey tests in recorded R3 gate | PASS |
| 8. Single ReviewPipeline entry, shared storage/store/limit/error, per-call client; stage facades public | `review.rs`, `pipeline/src/lib.rs`, README | Public pipeline journey and recorded R3 gate | PASS |
| Git identity, source-free persistence, store-clock durability, no new worker/provider/dependency, retained stage ownership | Shared listed-PR conversion validates base repository for both listing APIs; snapshot/facade and pipeline boundaries; cumulative manifests and diff | Focused provider test passed in this review; recorded R3 gate and diff check | PASS |

## R3 closure and full-range assessment

- **FIND-ranker-pipeline-2 closed.** An incremental read may finish only after joining the prior boundary or exhausting the listing; at ten pages with more advertised, `UnprovedHistory` precedes both update and fallback publication. Cold reads can stop at ten proved eligible PRs. The focused `history::tests::update_with_ten_new_prs_at_the_page_bound_fails` passed (1 selected).
- **FIND-ranker-pipeline-4 closed.** `IndexedFileFacts::validate` rejects empty, duplicate, and unordered reference names before `SymbolIndex::load` replaces data. `Snapshot::decode` rejects the same malformed persisted facts as corrupt. Focused core and snapshot tests each passed (1 selected); the public facade journey covers surviving old postings and valid round-trip in the recorded R3 gate.
- **FIND-ranker-pipeline-5 closed.** `listed_pull_request` requires the requested base repository ID; both closed and all-PR listing methods call it. The head may still be a fork. Focused `github::tests::repository_lookup_and_pull_request_listing_are_verified` passed (1 selected); cold-build no-publication proof is in the recorded R3 gate.

I found no material MISSING, INCORRECT, DRIFT, VIOLATION, or REGRESSION in the cumulative range. The R3 implementation changes existing shared owners and direct tests without adding a new abstraction or dependency. Exact focused commands were `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib history::tests::update_with_ten_new_prs_at_the_page_bound_fails -- --exact`, `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_repeated_and_unordered_references -- --exact`, `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot::tests::malformed_persisted_references_are_corrupt -- --exact`, and `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::repository_lookup_and_pull_request_listing_are_verified -- --exact`; all exited 0 with one selected test. The credentialed live evaluation remains unrun without a token; fake-provider and checked-in report proofs cover its local behavior. No proposed findings.

```citations
[]
```
