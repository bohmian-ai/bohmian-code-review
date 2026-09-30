# Domain review: ranking, packing, and eval arms

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `efc381f776e397ace4f43f9f4ab45fc4187436a5`
**Overall:** FAIL

## Boundary and coverage

Reviewed intent decisions 1–6 and R1 FIND-ranker-pipeline-1 against `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and `architecture/references/domain/pipeline-consumer-ergonomics.md`. Traced public facade `rank_with` and `build_candidate_context` into the four rankers and packing, pipeline PR ranking/packing into `SelectedSpan`, and caller supplied eval arms through `trial::evaluate`, `Evaluation`, `Summary`, and `live::run` report construction. Compared the cumulative diff with the base experiment and the checked-in report contract.

| Obligation | Evidence | Result |
|---|---|---|
| IO-free public rankers; S/C/E/F semantics; default F and empty-history C order | `select/src/rank.rs` scoring, sorting, exclusions; facade re-export and `rank_with`; owner and journey tests | PASS |
| Fractional candidate scores; context version 2 | `RankedFile`, `SpanKind::Candidate`, and pipeline `SelectedSpan` use `f64`; `CONTEXT_FORMAT_VERSION` is 2 | PASS |
| Co-change-only declaration spans in location order under file/token budgets | `candidate_spans` indexed-declaration fallback and `pack_candidates_into`; owner and public journey tests | PASS |
| Caller supplied named rankers, per-PR means, paired effects, seeded bootstrap, and correctly identified decision in emitted report | `Evaluation` and `Summary` compute by caller names; `live::run` emits fixed F/C decision labels for those same arbitrary arms | FAIL |

## Verification

At the immutable candidate, `mise run test:select` passed 15 tests and `mise run test:eval` passed 25 library tests plus the binary test. The custom-arm consumer test checks `Summary` and trial serialization; the live report test supplies only S/C/E/F and asserts the fixed F/C keys. No authenticated live run was available. The finding is independently reproduced by the public call path `live::run(&api, repos, Arms::new(&[("silent", ...), ("oracle", ...)], "silent", "oracle")?)`: `Summary` evaluates `oracle-silent`, then `record` emits its verdict under `repository_f_minus_c` and `run` emits its adoption under `adopt_f`.

## Material finding

### DOMAIN-RANK-1 — Generic evaluation report retains fixed experiment decision labels

**Classification:** INCORRECT; R1 FIND-ranker-pipeline-1 class incomplete. **Defect class:** experiment-specific labels and rule text remain in the generic report owner. The candidate-owned report shape permits the discrepancy.

**Root cause and siblings:** `live::run` accepts arbitrary validated `Arms`, but writes its decision to `adopt_f`; `record` writes the decision arm's verdict to `repository_f_minus_c` and writes `DECISION_RULE`, which explicitly describes F against C. The same record also gives every custom-arm run the fixed command and rarity/fusion experiment metadata. The first three locations falsely identify the actual decision; the latter metadata are siblings of the fixed-experiment reporting assumption. `Summary::new` itself uses the caller's baseline and decision correctly, so the defect begins when the generic live report is constructed.

**Violated obligation:** intent decision 3 and R1 FIND-ranker-pipeline-1 require named caller rankers to flow through decision output and report, while preserving the existing S/C/E/F report for the thin binary.

**Observable consequence:** a public consumer can evaluate `oracle` against `silent` and receive a report saying F was adopted against C. A reader or report ingester cannot tell which arms the decision actually compared; the report is factually wrong even though its numeric effects are correct.

**Testable correction:** make the generic report identify the selected baseline and decision arms and report their decision under accurate names or neutral fields. Keep the exact existing keys and values for the fixed S/C/E/F live binary. A fake-provider report test with non-C/F arms must assert that the emitted decision names and rule match those arms, while the checked-in C/E/F report regression remains byte-for-byte stable.

```citations
[
  {"id":"DOMAIN-RANK-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":359,"snippet":"    arms: Arms<'_>,","symbols":["run","Arms"]},
  {"id":"DOMAIN-RANK-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":383,"snippet":"            record[\"decision\"][\"adopt_f\"] = json!(adopt);","symbols":["run"]},
  {"id":"DOMAIN-RANK-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":462,"snippet":"            \"rule\": DECISION_RULE,","symbols":["record","DECISION_RULE"]},
  {"id":"DOMAIN-RANK-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":464,"snippet":"            \"repository_f_minus_c\": verdict,","symbols":["record"]},
  {"id":"DOMAIN-RANK-1","path":"crates/bohmian-code-review-eval/src/report.rs","line":28,"snippet":"pub const DECISION_RULE: &str = \"adopt F only if, in every repository, the mean per-PR F - C hit@10 difference is at least 0.05 and its 95% bootstrap interval's lower bound is above 0; an interval containing 0 is inconclusive\";","symbols":["DECISION_RULE"]}
]
```
