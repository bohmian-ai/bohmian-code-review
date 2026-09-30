# TASK-002 verdict

**Verdict: FIX_REQUIRED**

## Immutable subject

- Repository: `/Users/stevenforrester/Documents/GitHub/bohmian-code-review`
- Base: `6c7f9d3aef831f66510bc8c9c6ad553b277f7b2d`
- Candidate: `03664569e52baa96e86c71026fdfb68a5f9502b1`
- Authority: approved `changes/active/review-helper-setup/spec.md` revision 2 and the original `changes/active/review-helper-setup/tasks/TASK-002-index-resolver.md` at the base commit.
- Reviewed range: the complete six-file base-to-candidate diff. The separate uncommitted `IMPLEMENTATION_PLAN.md` edit is excluded. HEAD remained the candidate throughout both waves.

## Acceptance matrix

| Obligation | Implementation and verification evidence | Result |
|---|---|---|
| Exact case-sensitive simple and qualified lookup; duplicates, unknown names, deterministic ordering | `SymbolIndex` postings and lookup; three passing index tests | PASS |
| Complete per-file replacement, unrelated-file isolation, idempotent removal, fact and byte-length retention | `SymbolIndex::upsert`/`remove`, `IndexedFile`; passing index owner lane | PASS |
| All exact reference, qualified-candidate, and changed-declaration matches; ambiguity and repeated-name handling | Resolver and real-index test; passing resolver owner lane | PASS |
| IO-free crate boundaries, partial-coverage honesty, no source blob, parser coupling, persistence, fuzzy matching, or exact-definition claim | Production diff, manifests, source inspection, lints | PASS |
| Repository test placement rule | External index and resolver owner tests do not enter through the public facade | FAIL: FIND-TASK-002-1 |
| Exact focused-command evidence for every named test | Task evidence records broad lanes but no exact focused command or one-test selection | FAIL: FIND-TASK-002-2 |
| Required task checks | Independently rerun `test:index`, `test:resolve`, `fmt:check`, `lints`, and immutable diff check; all passed | PASS |
| Public journey and malformed-extraction transaction | Original task explicitly assigns these to TASK-003 | DEFERRED |

## Review results and validated findings

| Wave | Result | Report |
|---|---|---|
| Task implementation | PASS; no findings | `task-review.md` |
| Repository standards | FAIL; STANDARDS-001 and STANDARDS-002 | `standards-review.md` |
| Index-state/source-trust domain | PASS; no findings | `domain-review-index-state.md` |
| Independent Ponytail validation | COMPLETE; two retained findings | `findings-validation.md` |

| Finding | Validation | Required correction |
|---|---|---|
| FIND-TASK-002-1 | CONFIRMED, VIOLATION | Move both owner test bodies, helpers, and assertions into inline test modules in their owning crates; preserve their real cross-crate calls and rerun owner lanes. |
| FIND-TASK-002-2 | REVISED, VIOLATION | Run and record exact focused Cargo commands with package, final library target, exact selector, and observed one-test result for each of the four named tests. Label these current GREEN proof; do not invent historical focused RED evidence. |

No prior findings exist for this first TASK-002 review. The candidate's RED claims are historical evidence, not independently replayable from this commit. The public journey and extraction-failure isolation remain TASK-003 obligations. The two bounded corrections are packaged in `TASK-002-R1-test-proof.md` for `$cr-implement`.
