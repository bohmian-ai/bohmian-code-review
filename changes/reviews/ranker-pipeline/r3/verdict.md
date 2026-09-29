# Ranker pipeline task review R3

**Verdict: FIX_REQUIRED.** Immutable subject: base `322a2b9e0984062625d54274798a9c32cb01142d` through candidate `e52510f112b822f7f7692535b26ae81e53ac4b03`. `HEAD` remained the candidate throughout review. Authority: `changes/reviews/ranker-pipeline/intent.md` (each decision is an acceptance obligation), `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and the R1/R2 remediation tasks. There is no original task file by maintainer direction. The untracked `changes/active/impact-map/` is outside the subject and was untouched.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. One public IO-free `Ranker` with S/C/E/F semantics | `select/src/rank.rs`, facade re-export | Selector, eval, and public symbolic journey in gate | PASS |
| 2. Fused default; empty history orders as rarity | `DEFAULT_RANKER`, `Fused::rank` | Selector and public journey tests | PASS |
| 3. Feature-gated caller sample and named rankers through decision/report; fixed C/E/F report and thin binary | `eval/src/{trial,report,live}.rs`, bin `context_relevance.rs` | Named-arm consumer, custom live report, checked-in report regression, eval lane | PASS |
| 4. Distinct source-free persisted reference/candidate names; snapshot v4 | `core/src/facts.rs`, `index/src/lib.rs`, snapshot load | Valid round-trip tests pass; malformed references are accepted (FIND-4) | **FAIL** |
| 5. Fractional scores; context v2 | `SelectedSpan.score`, context version | Selector and pipeline tests | PASS |
| 6. Co-change-only candidates pack declarations under budgets | Selector fallback packing | Selector and public symbolic journey | PASS |
| 7. Always-on, complete-or-fail persisted history; update extension; time-eligible F ranking | `CoChangeHistory::read`, snapshot/update publication, PR context | Existing cap test covers zero qualified PRs; ten-qualified incremental cap can publish partial history (FIND-2); listed PR identity can admit foreign data (FIND-5) | **FAIL** |
| 8. One `ReviewPipeline` with one storage/store/limits/error, per-call client, public stages | `pipeline/src/review.rs`, public exports, README | Public pipeline journey and gate | PASS |
| R1: caller-named eval inputs, stable discovery, pipeline-owned provider IO | Eval arms and trial/report, shared history reader, `GitHubClient` | Eval/provider/history tests; history completion class remains open as FIND-2 | **FAIL** for history; other R1 outcomes PASS |
| R2: custom decision identity; fixed F/C report compatibility | `Arms` key/rule methods, `live::record` | Custom-arm report and checked-in regression | PASS |
| Repository rules and non-goals: identity trust, validated facts, source-free storage, no symbolic IO/worker | Cumulative source and crate ownership | Gate passes, but FIND-4 and FIND-5 violate written validation/trust boundaries | **FAIL** |

## Independent review and verification

Wave 1 results: `task-review-a.md` **FAIL** (TASK-A-1), `task-review-b.md` **FAIL** (TASK-B-1), `standards-review.md` **PASS**, `domain-review-history.md` **FAIL** (HIST-1), `domain-review-rank.md` **PASS**, and `domain-review-provider.md` **FAIL** (DOMAIN-PROVIDER-1). `mise run review:verify-citations changes/reviews/ranker-pipeline/r3 e52510f112b822f7f7692535b26ae81e53ac4b03` exited 0; every finding citation returned `OK`, with no rejected citations (`citations.txt`). The claim-only handoff is `claim-ledger.md`. Independent reproduction, caller traces, prior-ID disposition, and corrections are in `findings-validation.md`.

`mise run gate` exited 0 in this review, covering formatting, Clippy, all component/eval and public journey tests, review scripts, skill sync, and workspace check. `git diff --check 322a2b9e0984062625d54274798a9c32cb01142d e52510f112b822f7f7692535b26ae81e53ac4b03` also exited 0. Focused Wave 1 index, eval, pipeline journey, and history tests passed. A credentialed live GitHub run was unavailable without a token; the blocking cases are reproduced through candidate source paths and existing fake-provider behavior, with focused missing proofs specified in the remediation task. Passing gates do not cover those cases.

## Validated findings and prior closure

| Stable ID | Status and gate | Root cause and consequence |
|---|---|---|
| `FIND-ranker-pipeline-2` | `CLASS_INCOMPLETE`; task contract and repository standard | Incremental history accepts cold-depth proof at the discovery cap without reaching the prior boundary, then drops prior history and may publish a partial snapshot. |
| `FIND-ranker-pipeline-4` | `CONFIRMED`; task contract | Persisted reference names bypass fact validation; malformed names load and can alter postings. |
| `FIND-ranker-pipeline-5` | `CONFIRMED`; source trust and repository standard | Shared listed-PR decoding drops base repository identity, so history and evaluation can accept an unverified entry. |

Prior `FIND-ranker-pipeline-1` is closed by R2's caller-aware decision labels and fixed-output regression proof. Prior `FIND-ranker-pipeline-3` is closed by R1's pipeline-owned provider reads; FIND-5 is a separate missing identity check inside that owner. TASK-A-1 is a **note**, not a remediation: `architecture/bohmian-code-review-design.md:22` expressly requires same-tree snapshot and history reuse. No other note or rejected claim remains.

The bounded correction is `ranker-pipeline-R3-complete-boundaries.md`. Re-review must inspect the complete cumulative range after implementation.
