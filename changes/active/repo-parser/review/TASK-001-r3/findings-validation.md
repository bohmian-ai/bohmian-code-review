# TASK-001 r3 — findings validation (Wave 2)

Subject: base `9715440d`, candidate `1fbcc4a` (TASK-001 + R1 + R2, cumulative). Approved task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`. Diff: `/tmp/task001-r3-diff.patch`. Claim ledger: `/tmp/task001-r3-claim-ledger.md`. Wave 1 reports were not read; conclusions come from source and the cited rules.

## Merged claim table

| Claim | Reporter | Root cause | Merged with | Gate tested | Result |
|---|---|---|---|---|---|
| REPO-1a (per-scenario RED) | repo-rev | evidence record aggregates RED | — | (c) repository standard | REJECTED |
| REPO-1b (tracked/untracked diff inspection not recorded) | repo-rev | same evidence section | split from REPO-1 | (c) repository standard | NOTE |
| REPO-2 | repo-rev | spec.md:4 `status: draft` while task and authorities say "approved" | prior r1/r2 note (spec.md:4 draft) | (c) repository standard | NOTE (pre-existing at base, outside task write set; needs a human spec decision) |
| TASK-A-1 | task-rev A | `DeclarationFact::validate` does not reject non-declaration kinds | prior r1/r2 note (same observation) | (a) production path, (b) task contract, (d) absolute boundary | NOTE |

No two claims share a root cause with each other; REPO-2 and TASK-A-1 each share a root cause with a prior non-blocking note, not with a prior FIND-*. No FIND-* is reopened.

## Per-claim validation

### REPO-1a — RED recorded as a single compile failure, not per scenario

Rule verbatim, `architecture/references/languages/spec-driven-development.md:206-208`: "**RED** — add or select one test, run its exact focused command, and confirm it fails for the expected missing behavior rather than setup, compilation, or an unrelated defect." `:218`: "Record the expected RED failure, GREEN result, and final verification." `:219-222` (exceptions): "A pre-existing regression test may supply RED. When executable behavior already exists, report a verification-only or no-op result instead of manufacturing production churn. Documentation, generated-artifact, and static obligations may use a credible non-runtime proof; do not add meaningless tests solely to imitate TDD."

Exception check and application:
- Both scenarios introduce brand-new public symbols (`export`, `load`, `path_language`, `rank_files`, `build_candidate_context`, `EXTRACTION_VERSION`, `IndexedFileFacts`, two `Error` variants). A Rust test that calls a symbol that does not exist cannot fail at runtime; the compile error naming the missing symbols *is* the expected missing behavior. The "rather than ... compilation" clause guards against unrelated compile breakage, which is not what task line 68 records: it lists exactly the missing symbols.
- The approved task itself defines RED at lane granularity for both scenarios (task lines 30 and 40: "`mise run test:symbolic-context:journey` fails before the capability exists" / "fails on missing behavior"). The evidence matches the approved task's own RED contract.
- The R2 remediation task (`review/TASK-001-r2/TASK-001-R2-focused-test-commands.md`, "Decision-complete recommendation") explicitly directed: "do not retroactively claim these commands were used for RED (the recorded RED, a compile failure of the journey target, is accurate and stays as written)". The candidate followed that instruction.
- The authorities-update row (task line 72) is a documentation obligation; `:220-222` permits a non-runtime proof, and the row cites one.

Reproduction: task line 68 read; the nine symbols listed are all first introduced in this diff (`grep` of `/tmp/task001-r3-diff.patch` shows each as an added `pub` item). Gate (c) not met: no cited sentence requires one RED line per scenario, and the compile failure satisfies "expected missing behavior" for new public API. Ladder: nothing to change. **REJECTED.**

### REPO-1b — tracked/untracked diff inspection not recorded

Rule verbatim, `architecture/references/languages/testing-workflows.md:19-20`: "Run Cargo-backed checks sequentially. Record exact RED/GREEN results, broader lane results, `git diff --check`, and tracked/untracked diff inspection." Related `implementation-execution.md:245-246`: "5. `git diff --check`; 6. final diff inspection."

Reproduction: task lines 68, 78, 94 record `git diff --check`, the commit range, and a tracked-diff observation ("`Cargo.lock` changed only to add the facade's `serde_json` dev-dependency"); no sentence records an untracked-file inspection. `rg -n 'untracked' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` returns nothing.

Independent verification of the underlying state: `git status --short` at candidate shows only `?? changes/active/repo-parser/review/TASK-001-r3/` (this review's own scratch directory); `git diff --check 9715440d..1fbcc4a` exits 0; `git diff --stat 9715440d..1fbcc4a -- . ':!changes/active/repo-parser/review'` lists 14 files, all within the task's expected write set (six crates, journey test, three authorities, task file, `Cargo.lock`).

Gate: the sentence is a recording checklist item; the reviewed subject is an immutable commit range, so untracked files cannot be part of it, and the tracked-diff inspection is partially recorded (line 94). No observable consequence for the candidate. Distinguished from FIND-TASK-001-2, whose rule is a "must" with a replayability consequence. Classified **NOTE**; one-line remedy if the orchestrator wants it recorded: append "tracked/untracked diff inspection: `git status --short` clean apart from review scratch; write set matches the task" to the "Commands run" line.

### REPO-2 — authorities say "approved" while spec.md:4 reads `status: draft`

Rule verbatim, `spec-driven-development.md:128-130`: "Use `draft`, `approved`, and `superseded` status. Only explicit human approval moves a draft to approved. A material edit increments the revision and returns the spec to draft. An approved spec has no unresolved material decision." `:133-134`: "`cr-plan` derives implementation tasks only from an approved spec revision or from validated task-review findings under that same spec."

Source facts:
- `git show 9715440d:changes/active/repo-parser/spec.md | head -12` → `revision: 10`, `status: draft`, and line 9: "Revision 9 was approved; revision 10 incorporates the maintainer's corrected library-only boundary and awaits approval." Identical at candidate; the diff does not touch `spec.md`.
- The task existed at base with `spec: SPEC-repo-parser@10` and line 16 already instructing: "Update `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md` to distinguish those six crates from the newly approved outer IO-capable library before implementation." The implementer's "approved" wording in `AGENTS.md:23` and `bohmian-code-review-design.md:5` executes that instruction verbatim.
- `AGENTS.md:3` ("`changes/active/` holds approved change specifications") is unchanged from base.

Exception / introduction check: the diff did not move any spec status, so `:128-129` is not violated by the diff. The inconsistency (task planned from and referring to a rev-10 spec whose file still says draft) pre-exists at base and is a planning-stage condition under `:133-134`, outside TASK-001's expected write set (task line 52). Fixing it correctly requires a human act: the maintainer approving rev 10 (`spec.md:4` → `approved`), or, if rev 10 is not approved, the packet should not be proceeding at all. Neither is the implementer's decision.

Gate: (c) not met by the diff; task contract (b) satisfied, since the task ordered the wording. **NOTE**, carried forward from r1/r2, with an explicit escalation: before `cr-complete`, the maintainer must record rev-10 approval in `spec.md:4` (and line 9) or the "approved" wording in the two authorities must be re-derived from the actual status. This does not block TASK-001 because it is outside the task and needs a spec decision; if the orchestrator treats packet lifecycle integrity as a retained finding it becomes SPEC_REVISION_REQUIRED, not FAIL.

### TASK-A-1 — `load` accepts a non-declaration `DeclarationFact` and silently drops it from lookup/export

Obligation verbatim, `changes/active/repo-parser/spec.md:55` (REQ-001, last sentence): "Invalid paths or malformed facts fail explicitly without corrupting other indexed files."

Source path traced: `crates/bohmian-code-review-core/src/facts.rs:88-101` `DeclarationFact::validate` checks identity and span only. `crates/bohmian-code-review-index/src/lib.rs:156-165` `load` calls `facts.validate()?` then `upsert`; `upsert` (`:59-80`) skips `!is_declaration(node.kind)` for postings but keeps the node in `files[path].nodes`. `export` (`:127-142`) filters to declarations. Facade `SymbolicContext::load` (`crates/bohmian-code-review/src/lib.rs:193-204`) only adds the version check.

Reproduction (independent, `/tmp/taska1`, path-depends on core + index only; `cargo run`): a hand-built `IndexedFileFacts` with one `kind: Import` declaration →
```
load=Ok(()) lookup_os=0 nodes_kept=Some(1) exported_decls=0
```
Confirmed: Ok, not posted, not re-exported, node retained in the file's node list.

Gate application:
- (a) production path: not reachable from the facade's own `export` → `load` (export filters to declarations at index `:137`), so the realistic caller — the outer pipeline persisting facade export output and reloading it — never produces this input. A store would have to rewrite `kind` to exactly one of the four non-declaration serde tags. Common case not observably wrong.
- (b) task contract: REQ-001's tested malformed cases are identity and span (task scenario 1; journey test lines 465-472); an Import kind is treated exactly as `upsert` treats an Import node from the extractor, which is the documented contract ("exactly like `SymbolIndex::upsert`", index `:145-146`). No named acceptance criterion violated.
- (d) absolute boundary: no source trust, security, or durability impact; other files untouched (verified: the record is validated before any mutation).

Classified **NOTE** (same conclusion as r1/r2). If a later spec revision wants declaration-kind rejection at the trust boundary, the minimum correction is one branch in `DeclarationFact::validate` returning a new `InvalidFact::NotADeclaration { id, kind }`, which requires moving `is_declaration` from index into core (it is a pure function of `NodeKind`) and one journey assertion; not required by TASK-001.

## Final deduplicated blocking ledger

Empty — validated: every surviving claim was reproduced or traced from source above and none meets gate (a), (b), (c), or (d) as applied to the diff.

## Notes (non-blocking)

- REPO-1b: evidence section omits an explicit tracked/untracked diff inspection sentence; state verified clean by this review.
- REPO-2: `spec.md:4` still `draft` while task and authorities say approved; pre-existing at base, needs maintainer approval of rev 10 before `cr-complete`.
- TASK-A-1: `DeclarationFact::validate` does not reject non-declaration kinds; unreachable via facade export→load; behavior mirrors `upsert`.
- Carried from r1/r2, unchanged: `EXTRACTION_VERSION` hand-maintained; `prepare_changed` double UTF-8 decode; `pack_candidates` `max_files == 0` asymmetry; `Extractor::path_language` composite-route eligibility under restricted config (unreachable via facade).

## Rejected claims

- REPO-1a: compile failure naming the nine missing public symbols is the expected missing behavior for new public API; the approved task defines RED at lane level (lines 30, 40); the R2 remediation explicitly kept this RED wording. No cited sentence requires per-scenario RED rows.

## Prior-finding closure

- FIND-TASK-001-1 (task status out of enumerated set): `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` → `status: review` (in set per `spec-driven-development.md:171`). **Remains CLOSED.**
- FIND-TASK-001-2 (named tests without exact focused commands): task lines 80-92 carry nine `mise exec -- cargo test --locked -p <crate> --<target> <selector> -- --exact` rows covering all eight named tests plus both `facts::tests` tests. Each command re-run sequentially by this review: all nine exit 0, `1 passed; 0 failed`. **CLOSED.**

## Verification commands run

| Command | Observed |
|---|---|
| nine focused commands from task lines 84-92, run sequentially via `mise exec -- cargo test --locked ... -- --exact` | each rc=0, `test result: ok. 1 passed; 0 failed` |
| `git diff --check 9715440d..1fbcc4a` | exit 0 |
| `git status --short` | only `?? changes/active/repo-parser/review/TASK-001-r3/` |
| `git show 9715440d:changes/active/repo-parser/spec.md \| head -12` | `revision: 10`, `status: draft`, "awaits approval" |
| `git diff 9715440d..1fbcc4a --stat -- changes/active/repo-parser/spec.md AGENTS.md architecture/` | spec.md unchanged; AGENTS.md, agent-rules.md, design.md changed |
| `/tmp/taska1` `cargo run` (core + index path deps) | `load=Ok(()) lookup_os=0 nodes_kept=Some(1) exported_decls=0` |

## Verification limits

- `mise run gate`, `lints`, and `fmt:check` were not re-run by this review (time cap); the R2 evidence records them green and the nine focused commands plus `git diff --check` were reproduced here.
- TASK-A-1 reproduction used the index crate directly; the facade path was traced, not executed (facade `load` adds only the version check before delegating).

Overall: PASS

```citations
[]
```
