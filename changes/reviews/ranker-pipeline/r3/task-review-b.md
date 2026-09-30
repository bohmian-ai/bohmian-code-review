# Ranker pipeline R3 — independent task review B

**Subject:** base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `e52510f112b822f7f7692535b26ae81e53ac4b03`. **Authority:** `changes/reviews/ranker-pipeline/intent.md` decisions 1–8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and the R1/R2 remediation diagnoses and acceptance criteria. No original task file exists. I inspected the cumulative diff and candidate source without another R3 reviewer's conclusions.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. One public IO-free `Ranker` with S/C/E/F and experiment semantics | `select/src/rank.rs`; facade re-export | Selector and public journey cases; eval cases | PASS |
| 2. Fused default; empty history has rarity order | `DEFAULT_RANKER`, `Fused::rank` | Selector and symbolic journey cases | PASS |
| 3. Feature-gated reusable caller sample and named rankers; paired hit@10, bootstrap, decision and existing report | `eval/src/{trial,report,live}.rs`, thin binary | `mise run test:eval`: 26 library and 1 binary test passed, including checked-in reports and custom live decision names; R1 FIND-1 and R2 criteria | PASS |
| 4. Persist each file's distinct reference/candidate names without source positions; snapshot v4 | `core/src/facts.rs`, `index/src/lib.rs`, `pipeline/src/snapshot.rs`; loader does not validate the added field | `mise run test:index`: 5 passed, but malformed reference lists are untested and accepted; TASK-B-1 | FAIL |
| 5. Fractional selected scores; context v2 | `select/src/lib.rs`, `pipeline/src/context.rs` | Selector and context cases | PASS |
| 6. Co-change-only files pack declarations in location order under budgets | `select/src/lib.rs::candidate_spans` | Reasonless candidate packing test and public journey | PASS |
| 7. Bounded complete-or-fail history in build/update and F in review, excluding later/self merges | `pipeline/src/{history,snapshot,update,pull_request}.rs` | R1 cap/movement and facade tests in candidate | PASS |
| 8. One `ReviewPipeline`, storage/store/limits/error; cold build only without pointer; per-call client; public stage facades | `pipeline/src/review.rs`, public exports, README | Public pipeline journey and review owner tests | PASS |
| R1 provider reads belong to pipeline; eval has no raw provider path | `pipeline/src/github.rs`, `eval/src/live.rs` | Fake-provider discovery and error cases; source inspection | PASS |
| R1 history cap and moving multi-page listing fail before publication | Shared `CoChangeHistory::read`, snapshot/update callers | Focused history, cold-build and update cases | PASS |
| R2 custom report keys and rule name the selected decision pair; fixed F/C output preserved | `Arms::{rule,verdict_key,adopt_key}`, `live::{run,record}` | Custom fake-provider report and checked-in fixed report tests | PASS |
| Boundaries and non-goals: no symbolic IO/source retention, store-clock durable time, no worker, unchanged grammar pins | Crate owners, manifests, diff | Source inspection; `git diff --check` exited 0 | PASS |

## Proposed finding

**TASK-B-1 — INCORRECT / VIOLATION (intent decision 4 and `AGENTS.md` fact-loading rule).** Defect class: the new persisted reference-name field bypasses fact validation. `IndexedFileFacts::references` promises distinct names in ascending order, but `IndexedFileFacts::validate` checks only declarations. The public `SymbolicContext::load` calls `SymbolIndex::load`, which validates and then collects `facts.references` into a set. A caller can supply same-version facts with `references: ["save", "save"]`, `["z", "a"]`, or `[""]`; loading returns success and silently rewrites the record on export. An invalid reference can also become a `referencing` posting. This violates the written rule that loading malformed facts fails explicitly and leaves existing files intact; the new field is part of the approved persisted facts contract. The owner shape is a public raw `Vec<String>` checked by a validator that only knows declarations. Sibling paths are public facade loading, pipeline snapshot `into_context`, and snapshot reuse (`snapshot.rs` calls to `context.load`); they all route through the same fact validator. Correct once at `IndexedFileFacts::validate`, before index mutation, by rejecting empty, duplicate, or out-of-order reference names. A focused public-load test should show all three invalid cases error and preserve old postings and referenced names; a valid exported record must still round-trip. The existing index test covers valid references and invalid declarations only.

## Verification and result

`mise run test:eval`, `mise run test:index`, and `git diff --check 322a2b9e0984062625d54274798a9c32cb01142d e52510f112b822f7f7692535b26ae81e53ac4b03` exited 0. The credentialed live command was unavailable; the candidate's fake-provider and checked-in report tests cover its relevant behavior. No source was modified. Candidate was HEAD during inspection.

**Overall: FAIL.**

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-core/src/facts.rs","line":119,"snippet":"    pub references: Vec<String>,","symbols":["IndexedFileFacts"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-core/src/facts.rs","line":128,"snippet":"    pub fn validate(&self) -> Result<(), InvalidFact> {","symbols":["validate"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-index/src/lib.rs","line":218,"snippet":"        facts.validate()?;","symbols":["SymbolIndex::load"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-index/src/lib.rs","line":224,"snippet":"        let references = facts.references.into_iter().collect();","symbols":["SymbolIndex::load"]}]
```
