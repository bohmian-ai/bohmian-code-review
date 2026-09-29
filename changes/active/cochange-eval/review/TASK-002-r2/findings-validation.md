# Wave 2 findings validation

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`; approved `SPEC-cochange-eval` revision 2, original `TASK-002`, and remediation `TASK-002-R1`. The citation-checked Wave 1 claim ledger is empty. This validation used the cumulative diff and candidate source independently.

## Deduplicated blocking ledger

**Explicitly validated empty ledger.** No surviving Wave 1 claim requires reproduction, and the independent source check found no blocking defect under the production-path, task-contract, repository-standard, or absolute-boundary gates. No new `FIND-*` ID is assigned.

## Prior finding closure

**FIND-TASK-002-1 — CLOSED.** `Corpus::new` constructs `declarations` from every indexed base fact; `evaluate` retains all listed changes for C and changed-path exclusion; `trial` passes those remaining changes to E; and `rank_e` now filters anchors with `corpus.declarations.contains_key(&change.file.path)` before counting eligible history. It also limits credited destinations to indexed, non-excluded paths. F consumes the corrected E order. The only `rank_e` caller is `trial`, so the membership rule covers every selected trial without changing C, selection, or reporting. The focused `trial::tests::absent_base_changed_path_anchors_no_cochange` test passed (1 selected, 23 filtered): an absent-base added path present in earlier history gives no E/F credit, while an indexed anchor still contributes. The earlier root-cause class is closed rather than reopened as `CLASS_INCOMPLETE`.

## Independent scope check and recommendation

The candidate keeps the five-trial cap and deterministic draw, retains C's rarity rule, computes E's co-change proportions and F's rank fusion over the base corpus, and emits only C/E/F withheld rank and hit@10 in each trial. `main.rs` passes history filtered for each base time; the diff changes the experiment example and task packet, not production ranking or the gate. Removed packing and legacy-arm work is consistent with the approved ranking-only experiment. No redundant R1 remediation code or material drift was found. **Recommendation: PASS.** No remediation task or optional change is recommended.

Verification limit: this Wave 2 pass independently ran the focused R1 test and inspected source and the cumulative diff; broader lane results belong to the orchestrator's verdict evidence.
