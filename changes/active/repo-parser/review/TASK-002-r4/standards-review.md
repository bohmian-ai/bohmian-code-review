# TASK-002 r4 — repository-standards review (`repo-rev`)

Subject: base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15` → candidate `801d9572cf6b0364b5ab33e858388d86f54cb91e` (verified `git rev-parse HEAD` = candidate).
Diff audited: `/tmp/task002-r4-code.diff` (cumulative: `crates/bohmian-code-review-pipeline/*`, `Cargo.toml`, `Cargo.lock`, `mise.toml`) with particular attention to `/tmp/task002-r4-r3fix.diff` (commits `1958e0b`, `801d957`).
Authorities read: `AGENTS.md`, `architecture/agent-rules.md`, `architecture/references/README.md`, `architecture/references/languages/{rust-core,testing-workflows,implementation-execution,spec-driven-development}.md` (spec-driven grep-routed for rustdoc/import/lane rules: none beyond the others), `architecture/bohmian-code-review-design.md` (dependency-direction and journey paragraphs, lines 20 and 38).
Not reviewed: task acceptance, optional improvements, Ponytail audit.
Mechanical lanes: read from `/tmp/task002-r4-lanes.txt` — `fmt:check` exit 0, `lints` exit 0, `test:pipeline` 17 unit + 2 doctests ok, `gate` ok.
Read-only verification run here: `mise exec -- cargo clippy --locked -p bohmian-code-review-pipeline --all-targets --all-features -- -W clippy::missing_docs_in_private_items -W clippy::missing_errors_doc -W clippy::missing_panics_doc -W missing_docs` (see §2 rustdoc row), `git diff --check 1842eaf..HEAD` (exit 0), `rg` sweeps for `std::…::` paths, `unwrap`/`expect`, `#[allow]`/`#[ignore]`, and plan/task/agent references.

## 1. Authority coverage

| Changed surface | Governing authorities |
|---|---|
| New crate above the facade; depends on `bohmian-code-review` only (`crates/bohmian-code-review-pipeline/Cargo.toml:12`) | `AGENTS.md:23`; `architecture/agent-rules.md:3`; `design.md:20` |
| No symbolic-stage re-implementation; no source persistence; no clone/checkout | `AGENTS.md:21,23`; `agent-rules.md:3,9` |
| New deps `ureq`, `sha1`; lockfile growth (`Cargo.toml:32-35`, `Cargo.lock`) | `AGENTS.md:27,34`; `implementation-execution.md:105-107` |
| Struct-owned workflows (`GitHubClient`, `Execution`), traits `HttpTransport`/`ObjectStorage`/`Persistence` | `AGENTS.md:27`; `agent-rules.md:10`; `rust-core.md:3` |
| Synchronous IO, no runtime | `AGENTS.md:28`; `rust-core.md:5` |
| Errors: `thiserror`; no unchecked `unwrap`/`expect` on input/IO | `AGENTS.md:30`; `rust-core.md:7` |
| Imports at module top, bare imported types in signatures (`persistence.rs` `fmt` import; `github.rs:1941` test body) | `AGENTS.md:31`; `agent-rules.md:11` |
| Rustdoc on every new/modified item incl. private, tests, five `mod tests`, `ApiBase`; `# Errors`; `# Panics` on `with_timeout` | `AGENTS.md:32`; `agent-rules.md:11`; `rust-core.md:7` |
| Domain values at trust boundaries (`ApiBase`, `TenantId`, `ObjectSha`, `ObjectKey`) | `AGENTS.md:29`; `agent-rules.md:7`; `rust-core.md:7` |
| No `#[allow]`/`#[ignore]`/weakened assertions; `read_bounded` fix + new test | `AGENTS.md:40`; `agent-rules.md:12`; `implementation-execution.md:224-232` |
| Tests inline in owning module; credential/network-free | `agent-rules.md:5`; `testing-workflows.md:15`; `scripts/check-test-placement.sh` (run by `lints`) |
| `compile_fail` doctests on `GitHubHost` | `agent-rules.md:12`; `implementation-execution.md:224-232` (a check must prove the intended behaviour) |
| `mise.toml` `test:pipeline` list-form lane with `--doc`; `test:rust`/`gate` wiring | `AGENTS.md:40`; `testing-workflows.md:15`; `implementation-execution.md:177-179,250-259` |
| `--all-features` on cargo commands in lanes | `testing-workflows.md:15` (lane convention), sibling lanes `mise.toml:34-52` |
| Grammar pins untouched | `AGENTS.md:33`; `agent-rules.md:6` |
| No plan/task/agent references in production code | `AGENTS.md:44` |
| Task evidence doc (`tasks/TASK-002-github-source-and-stage-contracts.md`, R3 section) | `implementation-execution.md:113-131,294-340` (execution record / report shape) |

## 2. Rule-by-rule result

| Rule | Result | Evidence |
|---|---|---|
| Pipeline crate consumes only the public facade (`AGENTS.md:23`; `agent-rules.md:3`) | PASS | `crates/bohmian-code-review-pipeline/Cargo.toml:12` is the only workspace dependency; `lib.rs:9-10` states it. |
| No duplicate extraction/index/resolve/score/pack (`AGENTS.md:23`) | PASS | Crate content is identities, transport, storage/persistence traits, execution bounds; no tree-sitter, postings, scoring, or packing. |
| No source persistence, no clone/checkout (`agent-rules.md:9`) | PASS | `ContextRecord`/`SnapshotPointer` are keys, digests, shas only; `Archive`/`blob` bytes are transient return values. |
| Forbidden deps (`AGENTS.md:34`) | PASS | `Cargo.toml:35` `ureq` (rustls, platform-verifier, default-features off) and `sha1` are HTTP/hash crates, not a Git provider SDK, model SDK, Node/NAPI/SQLite/PostgreSQL/PyO3. Dependency addition is spec-owned (`implementation-execution.md:105-107`); spec approval is outside this review. |
| Concrete owners; traits only for multiple impls or intentional boundary (`AGENTS.md:27`; `rust-core.md:3`) | PASS | `HttpTransport` has `UreqTransport` plus test transports; `ObjectStorage`/`Persistence` are the consumer seams `AGENTS.md:23` names. `ApiBase` is a validated domain value, not an abstraction layer. |
| Synchronous, no runtime (`AGENTS.md:28`) | PASS | Blocking `ureq`; no async. |
| `thiserror` errors (`AGENTS.md:30`) | PASS | `Interrupted`, `InvalidIdentity`, `StorageError`, `PersistenceError`, `TransportError`, `GitHubError` derive `Error`. |
| No unchecked `unwrap`/`expect` on input, parsing, env, IO (`AGENTS.md:30`; `rust-core.md:7`) | PASS | `rg '\.(unwrap\|expect)\('` over `src/`: every hit is inside a `#[cfg(test)] mod tests` (`storage.rs:192-261`, `github.rs:1398-1723`, `identity.rs:453-507`, `persistence.rs:219-276`) or inside the `compile_fail` doctest text at `identity.rs:318`. Production `read_bounded` maps IO errors (`github.rs:1274-1275`). |
| Imports at module top; bare imported types in signatures (`AGENTS.md:31`; `agent-rules.md:11`) | PASS | `persistence.rs:12` `use std::fmt;` with `impl fmt::Display` / `fmt::Formatter` / `fmt::Result` (`persistence.rs:98-100`). `rg 'std::[a-z_]+::'` excluding `use` lines: single hit `github.rs:1941` `std::hint::black_box(usize::MAX)` — a path expression in a test body, not a `use` placement and not a signature; the written rule does not reach it. |
| Rustdoc on every new/materially modified item incl. private items and tests (`AGENTS.md:32`; `rust-core.md:7`) | PASS | Clippy `missing_docs_in_private_items` + `missing_docs` over `--all-targets`: zero warnings for any fn, method, struct, enum, trait, const, mod, or test in the pipeline crate; the five `mod tests` carry `///` lines (`execution.rs:92`, `github.rs:1367`, `identity.rs:430`, `persistence.rs:210`, `storage.rs:184`); `ApiBase` documented at `identity.rs:335-339`; new test documented at `github.rs:1937`. The lint's only output is "missing documentation for a field" on 51 private fields of private/internal structs (`execution.rs:47-48`; `github.rs:106,559-561,593-684,753-757`; `identity.rs:177-179,185-187`). Rust's grammar does not class a field as an item and the rule text says "item"; recorded as an interpretation note, not a finding. All `pub` fields are documented (`cargo doc -D warnings` is clean per the task record and no `missing_docs` hit is on a `pub` field). |
| `# Errors` on every fallible fn (`AGENTS.md:32`; `rust-core.md:7`) | PASS | Clippy `missing_errors_doc` produced no pipeline-crate warning; private `read_bounded` carries `# Errors` (`github.rs:1266-1270`). |
| `# Panics` where a panic remains (`agent-rules.md:11`; `rust-core.md:7`) | PASS | `Execution::with_timeout` documents the `Instant + Duration` overflow panic (`execution.rs:57-62`); `missing_panics_doc` produced no pipeline-crate warning. |
| Domain values at trust boundaries (`AGENTS.md:29`; `agent-rules.md:7`) | PASS | `Enterprise` payload is now `ApiBase(String)` with private field and no constructor (`identity.rs:330-340`); accessors read `.0` (`identity.rs:373,382`). |
| No `#[allow]`, `#[ignore]`, `#[expect]`, weakened assertion (`AGENTS.md:40`; `agent-rules.md:12`) | PASS | `rg '#\[(allow\|ignore\|expect)'` over `src/`: no hits. New `read_bounded_accepts_the_maximum_limit` asserts full body equality (`github.rs:1938-1943`). |
| `read_bounded` overflow removed rather than suppressed (`implementation-execution.md:224-232`) | PASS | `github.rs:1273` `(limit as u64).saturating_add(1)`; `rg 'as u64 \+ 1'` no match. |
| Tests inline with owning module (`agent-rules.md:5`) | PASS | No `crates/bohmian-code-review-pipeline/tests/`; `scripts/check-test-placement.sh` exit 0 in `lints`. |
| Lanes credential/network/Docker/checkout-free (`testing-workflows.md:15`) | PASS | `UreqTransport` is never exercised in tests; scripted transports only (`github.rs:1368+`). |
| `compile_fail` doctests prove the intended failure (`agent-rules.md:12`) | PASS | `identity.rs:310-312`: external struct-literal construction of a `#[non_exhaustive]` variant with a `String` payload where `ApiBase` is required — fails on both. `identity.rs:317-321`: `*api_base = String` with `api_base: &mut ApiBase` — E0308; `ApiBase` has no `From<String>`/`DerefMut`, so no alternative compile path. Lane output shows both `compile fail ... ok`. |
| `mise.toml` lane conventions (`AGENTS.md:40`; `testing-workflows.md:15`) | PASS | `test:pipeline` uses the same list-form `run = [...]` as `test:extract` (`mise.toml:33-36` vs `56-59`); both commands `--locked … --all-features`; the extra `--doc` invocation is required because `--all-targets` excludes doctests. `test:rust` depends on `test:pipeline` (`mise.toml:69`); `gate` depends on `test:rust` (`mise.toml:87`). |
| `--all-features` on cargo commands (`testing-workflows.md:15`; sibling lanes) | PASS | Both `test:pipeline` commands carry `--all-features`. |
| Grammar pins intact (`AGENTS.md:33`) | PASS | `Cargo.toml:39-54` unchanged in the diff. |
| No plan/task/agent references in production code (`AGENTS.md:44`) | PASS | `rg -i 'TASK-\|plan\|agent\|claude\|codex'` over `src/`: only `USER_AGENT`/`ureq::Agent` (`github.rs:34,106-118,1136`), which are HTTP terms. |
| No `.clone()` without ownership need (`AGENTS.md:29`) | PASS | Fix diff introduces no clone; `api_base.to_owned()` at construction is the ownership point (`identity.rs:365`). |
| `git diff --check` (`implementation-execution.md:247`) | PASS | exit 0 over `1842eaf..HEAD`. |
| Task evidence record shape (`implementation-execution.md:294-340`) | PASS | `tasks/TASK-002-github-source-and-stage-contracts.md` "TASK-002-R3 remediation evidence" carries AC table, non-goals, sequential command list, and write-set audit. |

Coverage limits: `test:pipeline`, `gate`, `fmt:check`, `lints` results are the orchestrator's (`/tmp/task002-r4-lanes.txt`), not re-run here. `Cargo.lock` was inspected only through `Cargo.toml` and the r3 review's dependency inventory; no new lockfile entries appear in the r3 fix diff. Spec approval of `ureq`/`sha1` is task-acceptance territory and was not adjudicated.

## 3. Findings

None.

Observations recorded for the maintainer (not findings; no written rule is violated):

- 51 private struct fields in the pipeline crate have no `///` line (locations in §2 rustdoc row). Sibling crates such as `bohmian-code-review-index` document private fields. If the maintainer reads `AGENTS.md:32` "item" with the scope of rustc's `missing_docs`/clippy's `missing_docs_in_private_items` (both report fields), this becomes a sweep across `execution.rs`, `github.rs`, and `identity.rs`. A one-line rule clarification in `architecture/references/languages/rust-core.md` would settle it either way.
- `github.rs:1941` calls `std::hint::black_box` by full path inside a test body. The import rule (`AGENTS.md:31`) governs `use` placement and signatures; a `use std::hint::black_box;` in the test module's import block would match the crate's own style but is not required by the text.

## 4. Overall

PASS

```citations
[]
```
