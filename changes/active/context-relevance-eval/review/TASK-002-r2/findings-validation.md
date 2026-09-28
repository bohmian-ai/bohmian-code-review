# TASK-002 Wave 2 findings validation

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `0aa3a607e770d3271f872417b535d555360b4a20`; approved `changes/active/context-relevance-eval/spec.md` revision 4. I inspected the cumulative evaluator diff, original TASK-002, remediation TASK-002-R1, governing rules, the prior `FIND-*` ledger, and the citation-checked r2 claim ledger. The candidate remained at the named commit. The surviving Wave 1 claims have one root cause.

## Blocking ledger

### FIND-TASK-002-3 — CONFIRMED

- **Wave 1 ID:** `TASK-A-1`. **Gate/classification:** Task contract; INCORRECT. The D rule in `spec.md:32` scopes a reference when its explicit binding identifies one indexed module, and permits an upstream target only if that module declares the name. TASK-002 Scenario 2 requires the same rule and C fallback only for unresolved or ambiguous imports.
- **Root cause / defect class:** The shared `score` owner treats a resolved module as unscoped if it lacks the queried declaration: `trial.rs:743-745` filters the binding through `declaring.contains_key` before deciding whether scoping applies. This is a wrong eligibility decision. The binding shape already represents a uniquely indexed module, so no new type or parser is needed.
- **Sibling locations:** Every D upstream lookup for a name bound to a unique indexed module without an indexed declaration reaches `trial.rs:743-759`; the repeated path is per remaining changed head and used name (`Change::new` at `trial.rs:610-623`, `Query::new` at `trial.rs:687-703`). There is one shared decision, not separate Python and Rust findings. D downstream uses a different check at `trial.rs:777-786` and does not have this root cause.
- **Independent reproduction:** `main.rs` calls `evaluate`; `evaluate` builds `Change` values and calls `trial` (`trial.rs:1027-1049`); `trial` calls `score(..., Rule::Scoped)` (`trial.rs:986-989`). Consider indexed `pkg/a.py` with `def other(): ...`, indexed `pkg/b.py` with `def wanted(): ...`, and a remaining changed head containing `from pkg.a import wanted` and a changed-line use of `wanted`. `bindings` resolves the explicit import to `pkg/a.py` (`trial.rs:390-423`, `:431-443`), while `corpus.declaring("wanted")` contains only `pkg/b.py` (`trial.rs:280-294`). The filter at `:744` turns the resolved module into `None`; the loop at `:753-761` credits `pkg/b.py` as an unscoped upstream hit. The required D rule has no eligible upstream declarer, so `pkg/b.py` must receive no D credit for this name. This can turn a withheld `pkg/b.py` from unranked to ranked and sent, altering its trial and aggregate results. The source path reproduces the defect without changing reviewed code. The existing focused trial lane passed five tests but has no case for a resolved module without the named declaration.
- **Smallest safe correction:** At `score`, retain the uniquely resolved module from `bound` even if `declaring` lacks that module. Let the existing upstream candidate filter admit only a declaration in that module; if none exists, credit no file. Keep C fallback for bindings that cannot identify one indexed module, C/D's original denominator, the df ceiling, and downstream scoping. This is a private evaluator correction; no spec revision, new dependency, or new abstraction is required.
- **Focused closure proof:** One no-network `evaluate` trial with the above imported name and an unrelated declarer asserts C can rank the unrelated declarer while D leaves it unranked (and counts the lookup scoped). Cover both Python and Rust only if the same shared fix has language-specific behavior; the owner is language-independent after `bindings`.

## Prior findings and redundant remediation

- `FIND-TASK-002-1` **CLOSED:** `trial.rs:757-760` uses the complete `declaring.len()` for C and D after the module eligibility decision. The exact-score and competing-rank test at `trial.rs:1353-1404` passes. The new finding concerns eligibility, not the denominator; reopening ID 1 would merge different root causes.
- `FIND-TASK-002-2` **CLOSED:** `bindings` checks import node line and column containment at `trial.rs:390-406`, shared by base candidate and remaining head producers. The same-line upstream, downstream, and repeated-name assertions at `trial.rs:1406-1485` pass. Its coordinate check and ambiguous-binding handling remain necessary; no prior remediation code is redundant.

## Rejections, notes, and limits

- `BUDGET-1` — **REJECTED by citation check** (`Corpus::declaring` symbol did not validate). `STD-R2-1` — **REJECTED by citation check** (`SymbolicContext::rank` and `Change::omit` symbols did not validate). They are not notes or blocking findings. `TASK-A-1` citations passed.
- No nonblocking notes. The confirmed finding is a named D contract violation, not a speculative edge case.
- Verification: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` passed 5 tests. The new failure is reproduced by the complete source path and concrete in-memory input, without adding a test or editing source during review. Broad gates were recorded by TASK-002-R1 but not rerun in this Wave 2 audit. No live GitHub evaluation was run; TASK-003 owns the paired live run.

**Recommendation:** `FIX_REQUIRED` for `FIND-TASK-002-3` at the private `score` owner.
