# TASK-001 Wave 2 validation (ponytail-rev)

**Subject:** base `9715440d` → candidate `34f4ae71` (HEAD), spec `SPEC-repo-parser@10`, task `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` (contract = sections above "## Implementation Evidence").
**Verified lanes (orchestrator):** `mise run gate` 0, `fmt:check` 0, `lints` 0, all owner lanes 0.
**Independent conclusions only:** no Wave 1 reasoning or review files read; ledger claims validated from source.

## Merged claim table

| Merged ID | Wave 1 sources | Claim (one line) | Status | Gate |
|---|---|---|---|---|
| M1 | TASK-B-1, DOM-FACTS-1 | `EXTRACTION_VERSION` is a free literal not mechanically coupled to grammar pins | NOTE | none |
| M2 | TASK-A-1 | `DeclarationFact::validate` accepts non-declaration `NodeKind`; `load` silently drops it from postings | NOTE | none |
| M3 | REPO-1 | Task frontmatter `status: implemented` is outside the enumerated task status set | CONFIRMED | (c) repository standard |

## Per-claim validation

### M1 (Claims 1 + 2, merged: same root cause, same constant) — NOTE

**Source read.** `crates/bohmian-code-review-extract/src/lib.rs:113-119`: rustdoc states the bump obligation ("Bump it whenever a grammar pin in `Cargo.toml`, a walker, the decoder, or the fact projection changes") and the constant is `"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1"`. `Cargo.toml:33-35`: `tree-sitter = "=0.25.10"` with the comment "A bump changes extracted facts and requires an approved specification revision." Consumer `crates/bohmian-code-review/src/lib.rs:193-198` refuses facts whose stamp differs (`Error::ExtractionVersionMismatch`). Journey `symbolic_context_journey.rs:455-460` proves the refusal path; `:404` proves export stamps with the constant.

**Gate analysis.**
- (a) Production path: at the candidate the stamp matches the pinned grammar (`0.25.10` == `Cargo.toml:35`); export→load round-trips and a foreign stamp is refused. Nothing is observably wrong on realistic input. Not met.
- (b) Task contract: acceptance "extraction changes invalidate old facts" is satisfied by the mechanism (`load` refuses a mismatched stamp). The contract does not require the stamp to be derived mechanically from pins; the task's Non-goals prohibit changing pins at all. Not met.
- (c) Repository standard: `architecture/agent-rules.md:9` requires "Exported facts ... stamped with the extraction version; loading facts from another version fails" — satisfied. REQ-002 ("changes when extracted facts or grammar behavior changes") is a maintenance obligation on future changes; no such change exists in the candidate to test against. Not met.
- (d) Absolute boundary: the harm needs a future pin bump that (i) violates the `Cargo.toml:33-34` comment requiring a spec revision, (ii) changes goldens (corpus tests at `crates/bohmian-code-review-extract/src/corpus_tests.rs:949`, `:1398` verify vendored grammar provenance; golden fixtures fail on walker/grammar output changes), and (iii) forgets the rustdoc-documented bump. The candidate itself breaks no durability path. Not met.

**Ladder on the proposed correction (test coupling stamp to pins).** A test asserting the stamp contains the `tree-sitter` pin string covers 1 of 13+ pins and none of walker/decoder/projection changes; it would give false assurance (theater). Deriving from `env!("CARGO_PKG_VERSION")` is the same convention with a different name (crate version is `0.1.0` and does not move with grammars). A build-time hash of golden corpus outputs would be a genuine mechanism but is a new build-artifact/identity decision (SPEC_REVISION_REQUIRED if ever wanted). Rung 1 holds: nothing to add for this task.

**Reproduction.** Not reproduced as a defect: `sed -n 35p Cargo.toml` → `tree-sitter = "=0.25.10"`; stamp string matches. No failing test can be written against the candidate.

**Disposition:** NOTE. Not a blocking finding; recorded as a maintenance limit already declared by the implementer.

### M2 (Claim 3) — NOTE

**Source read.** `crates/bohmian-code-review-core/src/facts.rs:88-103` checks identity emptiness and span only. `crates/bohmian-code-review-index/src/lib.rs:157-167` (`load`) validates then calls `upsert`; `upsert` (`:60-80`) skips posting for `!is_declaration(node.kind)` (`:62`) but retains the node in `files` (`:79`); `export` (`:127-141`) filters the same predicate. `is_declaration` at `:190-195`.

**Reproduction (scratch probe `/tmp/probe3`, path-depends on core + index):**
```
validate = Ok(())
load = Ok(())
file_count = 1
lookup(a.ts) = 0
nodes retained = Some(1)
export decls = 0
```
Loading a `NodeKind::File` fact is accepted; it is retained in `nodes()` and not posted; re-export drops it. This is byte-for-byte the behavior `upsert` gives the same node from a fresh parse (walkers emit `File` nodes; they are retained and never posted). No other file is touched.

**Gate analysis.**
- (a) `SymbolicContext::export` never emits a non-declaration fact (`export` filters at `:137`), so the public round trip cannot produce this input. Only a hand-edited or foreign record reaches it. Nothing observably wrong on the common case; nothing corrupted. Not met.
- (b) REQ-001 / Scenario 1 "malformed facts fail explicitly without corrupting other indexed files": the record is type-valid (valid `NodeKind`, one-based span, nonempty identity); the index handles it exactly as a parsed node. Reading "non-declaration kind" as "malformed" is a hardening interpretation, not a named criterion. Not met.
- (c) `agent-rules.md:10` ("malformed ... must not corrupt another file's entries") is satisfied. Not met.
- (d) No source-trust, security, or durability break. Not met.

**Ladder.** If ever wanted, the smallest change is a `kind` check in `DeclarationFact::validate` with a new `InvalidFact` variant; but `is_declaration` is owned by the index crate, so core would need the predicate moved or duplicated — an ownership decision the task does not require. Rung 1 holds.

**Disposition:** NOTE.

### M3 (Claim 4) — CONFIRMED, gate (c)

**Written rule.** `architecture/references/languages/spec-driven-development.md:171-173` (verbatim):
```
Task status moves through `proposed`, `ready`, `in_progress`, `review`, and
`approved`; use `superseded` when an approved spec revision or replacement task
invalidates it. Review phases remain read-only. The caller or active execution
```
This file is routed by `architecture/references/README.md` and is the repository's written contract for task documents (`AGENTS.md` "First pass before editing" step 2).

**Diff evidence.** `/tmp/task001-diff.patch:88-89`: `-status: proposed` / `+status: implemented`. Candidate `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:4` = `status: implemented`. Base was `proposed` (`git show 9715440d:...:4`).

**Reproduction.** `rg -n '^status:' changes/` → the only value outside `{proposed, ready, in_progress, review, approved, superseded}` (tasks) or `{draft, approved, superseded}` (specs) is this line. `.agents/skills/cr-implement/SKILL.md:73` returns the word `IMPLEMENTED` as a skill result, not as a frontmatter status; no skill authorizes `implemented` as a document status.

**Sibling sweep.** No other task file in `changes/` uses `implemented`. (`changes/active/repo-parser/spec.md:4` is `status: draft` while the task cites it as approved revision 10; that value is unchanged from base, outside this diff, and outside the task's write set — see Notes.)

**Ladder.** Rung 1: cannot delete the status line (required field, sdd.md:154). Rung 2: existing enumeration already has the right value. Correction is a one-token change.

**Disposition:** CONFIRMED, blocking under gate (c) only. Document-only; no code change.

## Retained blocking ledger

### FIND-TASK-001-1
- **Wave 1 source IDs:** REPO-1
- **Status:** CONFIRMED
- **Gate:** (c) repository standard
- **Classification:** documentation contract violation (task frontmatter)
- **Root cause:** implementer wrote a status value that is not in the enumerated task lifecycle set
- **Defect class:** enumerated-value violation in a spec-driven document
- **Sibling locations:** none (`rg -n 'status: implemented' changes/` → one hit)
- **Violated obligation:** `architecture/references/languages/spec-driven-development.md:171-173`
- **Evidence:** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:4` = `status: implemented`; introduced by diff hunk `/tmp/task001-diff.patch:88-89`
- **Reproduction:** `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` → `status: implemented`; `rg -n '^status:' changes/` shows every other task in-set
- **Observable consequence:** lifecycle tooling or a reader following sdd.md cannot place the task in a defined phase; the next transition (`approved` on PASS, or a remediation task) has no defined predecessor
- **Decision-complete correction (root-cause owner = task document):** change line 4 to `status: review` (implementation complete, immutable candidate under independent task review, per sdd.md:171 and cr-implement SKILL.md:75 "routes the immutable cumulative candidate to `$cr-task-review`"). No other line changes.
- **Focused closure proof:** `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` prints `status: review`; `rg -n 'status: implemented' changes/` exits 1 (no matches); `mise run gate` remains 0 (docs-only change).

## Notes (recorded, not blocking)

- **TASK-B-1 / DOM-FACTS-1 (M1):** `EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:119`) is a maintained constant; the rustdoc bump rule, `Cargo.toml:33-34` spec-revision requirement, and grammar-provenance/golden tests are the guard. A mechanical stamp (e.g. hash of golden outputs) would be a new build/identity decision — SPEC_REVISION_REQUIRED if ever pursued, not a task defect.
- **TASK-A-1 (M2):** `DeclarationFact::validate` (`crates/bohmian-code-review-core/src/facts.rs:88`) does not reject non-declaration kinds; `load` treats them exactly as `upsert` treats parsed non-declaration nodes (retained, not posted, dropped on re-export). Unreachable through the public export→load round trip.
- **Pre-existing, out of scope:** `changes/active/repo-parser/spec.md:4` is `status: draft` while the task and review treat it as approved revision 10; unchanged from base, not in the task write set.

## Rejected

- None. (No blocking claim failed reproduction; M1 and M2 were downgraded to notes by the gate, not rejected.)

## Verification limits

- Did not run `mise run gate` (orchestrator-owned; reported 0).
- M2 reproduction used a scratch cargo project under `/tmp/probe3` path-depending on `bohmian-code-review-core` and `bohmian-code-review-index`; no repository file was modified.
- Did not read `changes/active/repo-parser/review/TASK-001-r1/*review*.md` per instruction.

Recommendation: FIX_REQUIRED
