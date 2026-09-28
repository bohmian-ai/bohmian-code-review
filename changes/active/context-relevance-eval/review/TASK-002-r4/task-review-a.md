# TASK-002 Wave 1 implementation review A

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e2914e6d65a203519fefb47872040e1d2e454e20`; approved spec revision 4. I inspected the complete evaluator diff, original task, R1–R3 remediation and prior findings, applicable repository rules, evaluator callers, and source tests. The candidate stayed at the named commit.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: withhold one modified indexed file; keep its base version rankable; exclude remaining changed paths; use the full indexed base corpus and fail on missing base source | `trial.rs:233-277,864-872,961-1048`; `main.rs:220-276` | `withholding_keeps_base_file_rankable_and_a_is_production`, `variants_score_direction_rarity_and_imports`; `trial::` 7 passed | PASS |
| INV-002, AC-004: A uses public ranking and separate production packing; common comparison uses one experimental allowance | `trial.rs:825-862,905-1009` | A baseline and packing tests; `trial::` 7 passed | PASS |
| REQ-004–005: B directional names, C fixed rarity and ambiguity formula, D explicit unique import scope and fallback, deterministic rank | `trial.rs:282-312,366-487,675-817` | Variant, exact D credit, same-line Python, and resolved-import tests; `trial::` 7 passed | PASS |
| REQ-005, AC-003: ranks, names, directions, whole and span tokens, sent and omission states, remaining diff tokens and prompt reserve | `trial.rs:85-149,874-1009`; `main.rs:381-398` | Packing test; `trial::` 7 passed | PASS |
| REQ-006: all and non-test aggregates, hit rates, score distributions, and misses | `report.rs:43-113`; `main.rs:393-397` | `report::` 1 passed | PASS |
| R1/R2/R3 closure: exact D denominator, same-line binding, resolved scope without declaration, and parsed-patch/head inconsistency as a run error | `trial.rs:390-423,590-623,733-793` | Four focused regression cases in `trial::` 7 passed | PASS |
| INV-001, INV-003–005, AC-005 and non-goals: opt-in, source-free, read-only, no production API, snapshot, PR-context, replay, or gate change | Base-to-candidate diff changes evaluator example, task/review records, and `mise.toml` live-lane description only; `main.rs:339-401` | Diff inspection; `git diff --check` passed | PASS |

Ponytail audit: the added `Corpus`, `Change`, and `Query` hold distinct per-PR or per-trial state required by the comparison; `Bindings` and `score` share the B–D owner. The evaluator adds no dependency, production interface, compatibility layer, or speculative configuration. No material deletion preserves the task's complete measurement contract.

No proposed findings. Existing task and remediation evidence reports the broader pipeline, journey, format, lint, and gate lanes passing; this reviewer independently reran the evaluator `trial::` (7 passed), `report::` (1 passed), and diff whitespace check. No live GitHub run was available in this task review; checked-in reports and rerun reproducibility belong to the later live-run task. Overall **PASS**.

```citations
[]
```
