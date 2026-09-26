# Repository standards review: TASK-002

**Subject:** `6c7f9d3aef831f66510bc8c9c6ad553b277f7b2d..03664569e52baa96e86c71026fdfb68a5f9502b1` in `/Users/stevenforrester/Documents/GitHub/bohmian-code-review`. The reviewed HEAD remained `03664569e52baa96e86c71026fdfb68a5f9502b1`. The pre-existing uncommitted `IMPLEMENTATION_PLAN.md` edit is outside the immutable diff. No `.codegraph/` directory exists.

## Authority coverage

| Changed surface | Governing authority inspected |
|---|---|
| `index/src/lib.rs` | `AGENTS.md` ownership, Rust, source-lifetime, and trust rules; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md`; approved spec REQ-003/010 and INV-001/002/003/004/005; TASK-002 |
| `resolve/src/lib.rs` | Same repository and Rust authorities; design's ambiguity and owner rules; symbolic-context reference; approved spec REQ-004 and INV-002/004/005; TASK-002 |
| `index/tests/index.rs`, `resolve/tests/resolve.rs` | `AGENTS.md` test rules; `architecture/agent-rules.md` test-location rule; testing-workflows and spec-driven-development references; TASK-002 scenario and proof requirements |
| `mise.toml` | `AGENTS.md` verification rules; testing-workflows and implementation-execution references; approved spec REQ-009; TASK-002 owner lanes |
| TASK-002 implementation evidence | `AGENTS.md` change lifecycle; spec-driven-development and implementation-execution references; testing-workflows' exact-command rule; approved spec and original task |

The unchanged workspace manifests and `Cargo.lock` show index depends on core and resolve depends on core plus index. Existing selector and facade manifests consume these crates, but their source remains placeholder code. The complete six-file diff changes no grammar, generated artifact, or dependency. No additional architecture reference applies through the router.

## Applicable rule results

| Rule | Result | Evidence |
|---|---|---|
| Six crate ownership and downward dependency graph | PASS | Index owns `SymbolIndex`; resolver owns name matching; manifests retain the approved dependency direction. |
| Synchronous, IO-free, fact-only runtime; no coverage or exact-definition claim | PASS | Index retains `Vec<Node>` and `byte_len`; resolver returns all index hits; neither crate invokes filesystem, network, model, or parser code. |
| Canonical path value at ingress, ambiguity, exact ordering, and replacement isolation | PASS | `RelativePath` is the index key; `BTreeSet<Posting>` orders hits; `&mut self` replacement and remove act on one file. This is standards compliance, not an independent task-acceptance verdict. |
| Concrete state owner, borrowed input, imports, rustdoc, errors, and lint integrity | PASS | `SymbolIndex` is concrete; resolver's pure functions borrow facts; new items have intent docs; no new fallible API, unchecked unwrap, lint allowance, or grammar change. |
| Tests inline with owning module unless an external test crosses crate boundaries through the public facade | **FAIL** | Both new files are external crate tests. `index/tests/index.rs` exercises only index plus core facts; `resolve/tests/resolve.rs` crosses index/resolver but does not use the public facade. See STANDARDS-001. |
| Named Rust test evidence includes exact package, target, selector, and proven execution | **FAIL** | TASK-002 evidence names four tests but records only owner-lane commands and abbreviated results; it does not record exact focused commands. See STANDARDS-002. |
| Owner lanes, format, lints, and clean immutable diff | PASS | Independently reran `mise run test:index` (3 pass), `mise run test:resolve` (1 pass), `mise run fmt:check`, `mise run lints`, and `git diff --check base candidate`; all exited 0. Toolchain emitted vendored C warnings but Clippy passed. |
| Public journey and malformed-extraction transaction proof | DEFERRED | TASK-002 explicitly assigns facade behavior and the public journey to TASK-003; no facade output changed in this candidate. These are not TASK-002 standards failures. |

## Material findings

### STANDARDS-001 — Owner tests violate the repository's placement rule

- **Rule:** `architecture/agent-rules.md:5`: owner tests are inline by default; an external test earns its place by crossing crate boundaries through the public facade.
- **Location:** `crates/bohmian-code-review-index/tests/index.rs:1` and `crates/bohmian-code-review-resolve/tests/resolve.rs:1`.
- **Evidence and consequence:** Both files are external integration targets, although neither calls the public facade. The index tests are ordinary index-owner checks; the resolver test calls the index directly. This creates external targets outside the repository's stated test layout and leaves the real public journey to a different task.
- **Testable correction:** Place these owner checks inline with their owning modules and preserve their assertions and scoped mise lanes. Reserve an external test for the TASK-003 public facade journey. Run both owner lanes again.

### STANDARDS-002 — Exact focused test commands are absent from the task record

- **Rule:** `architecture/references/languages/testing-workflows.md:17-20` and `architecture/references/languages/spec-driven-development.md` require exact `mise exec -- cargo test --locked` commands for named Rust tests, with explicit package, target, and exact selector, plus proof that each selects a test.
- **Location:** `changes/active/review-helper-setup/tasks/TASK-002-index-resolver.md:90-106`.
- **Evidence and consequence:** Four test names appear in RED/GREEN and the acceptance map, but only `mise run test:index` and `mise run test:resolve` are recorded. Those lane results prove the tests pass collectively, not that the named focused selectors ran as required. The Red/Green audit trail is incomplete.
- **Testable correction:** After the test-placement correction, execute and record the exact focused command and one-test selection/result for each named test, then retain the owner-lane and format/lint evidence. Do not infer an unobserved RED result from the current green run.

**Overall: FAIL.** The runtime owner and boundary rules pass; two explicit test-workflow rules do not.
