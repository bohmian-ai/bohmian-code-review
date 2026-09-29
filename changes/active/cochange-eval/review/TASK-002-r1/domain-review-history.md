# Domain review: history timing and base-snapshot boundary

**Subject:** base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`
**Result:** FAIL

## Reviewed boundary

I traced the TASK-001 `History::build` and `History::for_trial` handoff through `Api::select`, `Api::trials`, `evaluate`, and E scoring. Each trial receives the history PRs with `merged_at < base_time`; the trial PR number is also excluded. Cold history has ten non-mass PRs before the earliest sampled base, and later history is filtered per base time. This preserves the approved time definition and prevents future PR leakage on the traced path.

I also traced base-snapshot coverage from `Snapshot::into_context` and verified source-file screening, changed-path exclusion, and E's candidate filter. E restricts credited candidate `f` to indexed base paths, but does not restrict the remaining changed source path `d` used to select history PRs.

## Authority and source coverage

| Authority | Boundary checked |
|---|---|
| `changes/active/cochange-eval/spec.md` History, INV-002, INV-003, AC-003 | Strict merge-before-base timing; absent base paths contribute nothing; fake-provider proof |
| `changes/active/cochange-eval/tasks/TASK-002-ranking.md` scenarios 1–2 and acceptance | Same base candidate universe, changed-path exclusions, time isolation |
| `AGENTS.md` product boundary and `architecture/bohmian-code-review-design.md` | Experiment stays in the outer pipeline example; symbolic facade remains the source of base facts |

Source inspected: complete base-to-candidate diff; `examples/context_relevance/{main,history,sample,trial,report}.rs`; applicable design and symbolic-context references. `mise run test:pipeline` and `git diff --check <base>..<candidate>` both exited 0. The focused history and trial tests cover future-time filtering, absent **candidate** paths, and the normal co-change formula.

## Material finding

### HIST-1 — An absent base path is allowed to originate E history credit

**Classification:** INCORRECT; violates the approved History rule that a history path absent from the trial's base snapshot contributes nothing, and TASK-002's base-snapshot candidate-universe boundary.

**Root cause and defect class:** `rank_e` counts history PRs touching every remaining changed file before checking whether that changed path exists in `corpus.declarations` (lines 419–424). The membership check at line 427 applies only to the credited path `f`. `evaluate` builds `Change` values for every listed eligible source file (lines 533–536), including an added file absent from the base snapshot. This is a missing source-side base-eligibility check in the E score owner; the candidate-side check does not cover it.

**Sibling sweep:** E's `rank_e` is the sole co-change scoring site. All trials reach it through `trial`; the shared `remaining` construction admits added paths. C and F do not independently consume history paths. The time filter in `Api::trials` is correct and does not repair this base-membership gap.

**Observable consequence:** Suppose the base has `b.py` and a withholdable modified `c.py`, but no `a.py`. The trial PR adds `a.py`, and an older eligible history PR touched `a.py` with `b.py` before `a.py` was removed. Withholding `c.py` leaves added `a.py` in `remaining`. E assigns `b.py` credit from that old history PR, even though `a.py` is absent from the trial base. E and F ranks can change on this reachable input. This is base-snapshot leakage, not future-time leakage.

**Testable correction:** In E, skip a remaining changed file whose path is absent from the indexed base corpus before finding touching history PRs. Add one fake/in-memory trial where an added, previously used path is absent from the base and verify it supplies no E or F credit while an indexed remaining path still does.

## Verification limits

The no-network tests passed, but they do not exercise a non-indexed remaining path `d` with matching historical co-changes. I did not run the credentialed three-repository command; this task review is of the immutable candidate and its no-network evidence. Provider timestamps and listings are trusted as the approved spec directs; their internal consistency was outside this review.

```citations
[{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":419,"snippet":"    for change in remaining {","symbols":["rank_e"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":423,"snippet":"            .filter(|paths| paths.contains(&change.file.path))","symbols":["rank_e"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":427,"snippet":"            if corpus.declarations.contains_key(path) && !excluded.contains(path) {","symbols":["rank_e"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":533,"snippet":"    let changes = listed","symbols":["evaluate"]}]
```
