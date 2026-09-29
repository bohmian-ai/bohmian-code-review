# Domain review: ranking and candidate boundary

## Subject and boundary

Base `3fecce95f055268359d7d1d87df6308ea504329e`; candidate `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`. Reviewed TASK-002's C/E/F scoring, deterministic trial choice, common base-snapshot candidate universe, changed-path exclusion, time-eligible history handoff, and ranking-only trial output. This is an experiment example, so production packing and model exposure are outside its result.

## Authority and source coverage

Read `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, `architecture/references/README.md`, the symbolic-context and testing references, approved `changes/active/cochange-eval/spec.md` revision 2, TASK-002, manifests, `mise.toml`, and the complete base-to-candidate diff. Traced `Api::trials` → `History::for_trial` → `evaluate` → `trial` → `rank_c`/`rank_e`/`rank_f` → `Outcome::of` and `record`, including the base corpus and `Screen` input path. Compared C's current rarity rule with the base scoring implementation and inspected the focused tests.

| Domain obligation | Evidence | Result |
|---|---|---|
| C retains directional rarity weighting, document-frequency ceiling 100, and path ties | `trial.rs:355-407`; base `score` with `Rule::Rare`; C regression test | PASS |
| E sums co-change proportions per remaining path, excluding absent base paths and remaining changes | `trial.rs:409-436`, `449-456`; history handoff in `main.rs`; E/F test | PASS |
| F fuses only nonzero C/E ranks using constant 60; zero-score files are unranked | `trial.rs:340-353`, `438-447`; E/F test | PASS |
| All arms use the same base corpus and trial; at most five deterministic withheld files | `trial.rs:482-552`; selection and exclusion tests | PASS |
| Trial outcome is rank or unranked and hit@10 only; no packing result | `trial.rs:59-116`, `482-507`; exact JSON test | PASS |
| History excludes PRs merged at or after each base commit and the trial PR | `history.rs:147-153`; `main.rs` history handoff; history and E/F tests | PASS |

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::tests:: -- --nocapture` passed: 6 tests, 0 failures. This review did not run the credentialed experiment or reproduce checked-in reports; those are later task/integration obligations. The test proves these example functions on in-memory cases, not empirical ranking benefit or production packing behavior.

## Material findings

None in this domain.

## Overall result

PASS.

```citations
[]
```
