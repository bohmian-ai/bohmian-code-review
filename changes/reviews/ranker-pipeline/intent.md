# Ranker trait, fused default, and ReviewPipeline — review authority

Human-approved design for commits `322a2b9..065f5ea` on `change/cochange-eval`. There is no spec; the maintainer directed this work directly after the co-change experiment (`changes/completed/2026/cochange-eval.md`) chose F. This file records those decisions so the work can be reviewed against them.

## Decisions

1. **Ranker trait in the public crates.** One `Ranker` trait in `bohmian-code-review-select`, re-exported by the `bohmian-code-review` facade, IO-free. Implementations: `SymbolOverlap` (the previous production ranker), `RaritySymbol` (C), `CoChange` (E), `Fused` (F, reciprocal-rank fusion, k = 60). C, E, and F keep the experiment's exact semantics and regression expectations.
2. **Default ranker is `Fused(RaritySymbol, CoChange)`.** With empty history it orders exactly as `RaritySymbol`.
3. **Evaluation crate behind a feature.** `bohmian-code-review-eval`, built only with its `eval` feature, is a reusable hit@10 harness that takes a repository sample and named `&dyn Ranker`s, computes per-PR means, paired differences, seeded bootstrap, and the decision rule, and emits the existing report shape. `mise run live:context-relevance` is a thin binary over it. The old `examples/context_relevance` is removed. A live run after the move reproduced the checked-in C/E/F reports exactly.
4. **Persisted reference names.** Each indexed file's distinct reference and extractor-candidate names (names only, no positions or source) are persisted; `SNAPSHOT_FORMAT_VERSION` 3 → 4.
5. **Fractional scores.** `SelectedSpan.score` is `f64`; `CONTEXT_FORMAT_VERSION` 1 → 2.
6. **Co-change-only candidates pack** their indexed declarations in location order under the existing budgets.
7. **Co-change history in both passes, always on.** Pass one (cold build and update) reads bounded, complete-or-fail co-change history (mass edits over 30 files dropped) and persists it with the snapshot as PR number, merge time, and changed paths only. Update extends it with PRs merged since the previous snapshot. Pass two (PR context) always ranks with F over the persisted history, excluding the PR itself and PRs merged after its base commit time. No opt-in path; empty history is valid.
8. **One consumer entry point.** `ReviewPipeline` in the pipeline crate, as specified in `architecture/references/domain/pipeline-consumer-ergonomics.md` including its review checklist: one storage, one store implementing both `SnapshotStore` and `ContextStore`, one `PipelineLimits` (one shared `max_file_bytes`), one `PipelineError`; `context` cold-builds only when no snapshot exists; `refresh` wraps update; `read` needs no GitHub access; the client is a per-call argument. Stage facades stay public.

## Unchanged authority

`AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md` (as updated in this range) still apply: crate ownership, no source persistence, store-clock time, rustdoc, error, and test rules, and the public-library journeys.
