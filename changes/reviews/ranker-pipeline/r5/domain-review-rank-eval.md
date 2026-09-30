# Rank and evaluation domain review

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..9efafd309207c6cf5dd96b306ce8d22325281e5d` (HEAD confirmed at candidate). **Result: PASS.** No material finding in this domain.

## Boundary and authority

Reviewed the complete cumulative range for symbolic ranking, co-change candidate packing, evaluation trials, inference and report output, and the R4 typed live error path. Authority: `changes/reviews/ranker-pipeline/intent.md` decisions 1–3 and 5–7; R1–R4 remediation tasks; `architecture/references/domain/pipeline-consumer-ergonomics.md`; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/references/languages/rust-core.md`. There is no original task file by maintainer direction.

## Source trace and checks

| Boundary | Source traced | Evidence and result |
|---|---|---|
| Rankers and default | `select/src/rank.rs` `RankInput`, `SymbolOverlap`, `RaritySymbol`, `CoChange`, `Fused`, `ordered`; facade `rank_with` consumers | Indexed-only and changed/previous-path exclusions apply. Rarity uses distinct directional names and document frequency; co-change anchors base-indexed paths; fusion uses `k=60`. Empty co-change leaves rarity order. PASS. |
| Candidate packing | `select/src/lib.rs` `pack_candidates_into`, `candidate_spans`, `slice_lines` | Co-change-only reasons produce indexed declarations ordered by location; each selected span counts its supplied tokens, and file/token ceilings remain checked. PASS. |
| Reusable evaluation | `eval/src/trial.rs` `evaluate`, `trial`, `remaining`; `eval/src/report.rs` `Arms`, `Evaluation`, `pr_means`, `effect`, `verdict`, `Summary` | Caller-named rankers run on the same withheld-file input; means weight PRs equally; seeded bootstrap and decision use selected arm identities. The R1/R2 custom-arm and fixed-report regressions pass. PASS. |
| Typed live workflow | `eval/src/live.rs` `LiveError`, `Api::{base_snapshot,qualify,trials,select}`, `live`, `run`, `print`, and history closure; binary `main` | Every R4 sibling returns `LiveError`; provider, history, snapshot, sample, symbolic, trial, identity, transport, arms and serialization causes remain typed. Configuration failure is a distinct variant. Fake-provider tests match failures without downcasting. Binary still prints error and exits 1. FIND-ranker-pipeline-6 closed. PASS. |

`mise run test:eval` exited 0: 27 library tests and 1 binary test passed, including the custom-arm report, checked-in C/E/F reports, typed fake-provider errors, malformed repository configuration, and source boundary test. `git diff --check` for the cumulative range exited 0.

## Limits

No credentialed live GitHub evaluation ran; none was authorized or supplied. This review used the fake-provider path and checked-in report reconstruction. Pipeline history completeness and source trust are covered by separate domain reviewers; this report traced their ranking/evaluation consumption only.

```citations
[]
```
