# Domain review: index state and source retention

**Subject:** `6c7f9d3aef831f66510bc8c9c6ad553b277f7b2d..03664569e52baa96e86c71026fdfb68a5f9502b1` (`TASK-002`), reviewed without changing production source.

**Boundary reviewed:** caller-supplied canonical path and extracted `Node` facts enter `SymbolIndex`; per-file replacement/removal update postings; `resolve` consumes exact hits. Extraction-failure handling in the facade is assigned to TASK-003 by the original task.

| Obligation and authority | Source evidence | Verification and result |
|---|---|---|
| Canonical caller path; `AGENTS.md`, `architecture/agent-rules.md`, spec INV-003 | `RelativePath::new` validates path spellings; `SymbolIndex::upsert` takes `RelativePath` (`core/src/relative_path.rs`, `index/src/lib.rs:57`) | Constructor boundary is enforced by type; PASS |
| Per-file complete replacement and idempotent removal; spec REQ-003, AC-003, design and symbolic-context reference | `upsert` removes prior file before inserting its new facts/postings; `remove` deletes that path's postings and returns false for an absent path (`index/src/lib.rs:57-98`) | `upsert_replaces_postings_and_removal_is_idempotent` passes; PASS |
| Other files remain isolated; spec REQ-003, AC-003, agent rules | Posting removal retains all paths other than the removed file (`index/src/lib.rs:90`); file map mutation is keyed by canonical path | Same replacement test retains the other file's hit and byte length; PASS |
| Facts and size only, no source retention or IO; spec REQ-003, REQ-010, INV-001/003 | `IndexedFile` contains only `byte_len` and `Vec<Node>`; production index/resolve code has no source-byte parameter, file IO, network, persistence, or parser dependency (`index/src/lib.rs:21-27`, manifests) | `successive_upserts_retain_facts_and_byte_lengths` passes; source/manifests inspection; PASS |
| Deterministic exact hits and honest partial coverage; spec INV-002/004, design | `BTreeSet<Posting>` orders path, line, column, ID and slot; `lookup` returns borrowed `SymbolHit`s; docs limit coverage to supplied files (`index/src/lib.rs:30-43,100-115`) | `exact_lookup_returns_all_duplicates_in_canonical_order` passes; PASS |
| Ambiguous candidate handoff; spec REQ-004, INV-002 | `resolve` deduplicates query names and retains all `index.lookup` hits (`resolve/src/lib.rs:43-59`) | `all_name_sources_resolve_to_every_plausible_hit` passes; PASS |

**Source coverage:** complete base-to-candidate diff for index, resolve, their tests, mise lanes, and task evidence; core `Node`/`NodeKind` and `RelativePath`; current consumer call sites; approved spec and original base task; `AGENTS.md`, agent rules, design, and symbolic-context reference. No `.codegraph/` index exists. `mise run test:index` independently passed 3 tests; `mise run test:resolve` independently passed 1 test. The task evidence reports `fmt:check`, `lints`, and `git diff --check` passing; these were not rerun in this domain review.

**Limits:** `SymbolIndex` receives completed facts, not an extraction result, so malformed/unsupported extraction isolation is not reachable through this API. The approved task assigns that transaction to TASK-003. The current tests do not exercise concurrent readers, and the API's `&mut self` mutation excludes them during a replacement.

**Material proposed findings:** none.

**Overall: PASS.**
