# TASK-002 r3 — findings validation (ponytail-rev, Wave 2)

## Subject

- Base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `dbd6de34f49764b32b00628cc539e719914469ed` (HEAD); crate `crates/bohmian-code-review-pipeline`.
- Spec `changes/active/repo-parser/spec.md` rev 10; task `TASK-002-github-source-and-stage-contracts.md`; remediation tasks R1 and R2.
- Input: `claim-ledger.md` only. Other r3 reports not read. No tracked file modified.

## Merged ledger

| FIND | Wave 1 IDs | Status | Gate | Class | Correction owner |
|---|---|---|---|---|---|
| FIND-TASK-002-4 | TRUST-1 | CLASS_INCOMPLETE | 4 (source trust / credential forwarding) | validated domain value with public writable field | `identity.rs` `GitHubHost::Enterprise` payload |
| FIND-TASK-002-5 | TASK-A-2, TASK-B-1 | CONFIRMED | 2 (AC-R2-5 proof) + 3 (AGENTS.md:40, testing-workflows.md:15) | verification target absent from every lane | `mise.toml:56` `test:pipeline` |
| FIND-TASK-002-6 | REPO-1 | CONFIRMED | 3 (AGENTS.md:32, rust-core.md:7, agent-rules.md:11) | undocumented caller-input panic | `execution.rs:58`, `github.rs:1271` |
| FIND-TASK-002-7 | REPO-2 | CONFIRMED | 3 (AGENTS.md:32) | rustdoc omission on new items | five `mod tests` |
| FIND-TASK-002-8 | REPO-3 | CONFIRMED | 3 (AGENTS.md:31, agent-rules.md:11) | import discipline | `persistence.rs:97-99` |
| — | TASK-A-1 | NOTE | none | deferred to facade binding (r2-recorded) | TASK-003..005 facades |

### FIND-TASK-002-4 — CLASS_INCOMPLETE (TRUST-1)

- **Root cause:** r2 sealed *construction* of `GitHubHost::Enterprise` with `#[non_exhaustive]` (`identity.rs:318`) but enum variant fields are inherently public, so the *write* path is still open. The r2 task rejected "a newtype for the payload" as more surface (R2 task line "Rejected alternatives"); that rejection is what left the class open.
- **Siblings:** `identity.rs:318-322` (variant), `:353-358` (`api_base`), `:361-369` (`archive_host`); consumers `github.rs:1121` (`request` builds URL from `api_base()`), `:1139` (`build` attaches `Bearer` token), `:741-746` (`is_trusted_redirect` compares against `archive_host()`).
- **Violated obligation:** `identity.rs:307-308` own invariant ("every API base a client sees is validated"); spec REQ-014 `spec.md:77` ("Archive redirects and paths prevent credential forwarding"); REQ-004 `spec.md:61` (configured GHES instances validated); AGENTS.md:29 (domain values at trust boundaries); AC-R2-5 intent (FIND-TASK-002-4 closure).
- **Reproduction (`/tmp/t002r3`, external crate, path dep on candidate):**
  ```rust
  let mut h = GitHubHost::enterprise("https://ghe.example.com/api/v3").unwrap();
  if let GitHubHost::Enterprise { api_base, .. } = &mut h { *api_base = "http://evil.example".to_owned(); }
  assert_eq!(h.api_base(), "http://evil.example");   // passes
  assert_eq!(h.archive_host(), "");                   // passes
  ```
  `cargo test` → `trust1_external_mutation_forges_api_base ... ok`.
- **Observable consequence:** a consumer holding a validated host can (deliberately or via a generic "fix up config" path) point every authenticated request, including the `Authorization: Bearer` header, at an arbitrary cleartext host; `archive_host()` degrades to `""` so every archive redirect is then refused (fail-closed there, but the token has already been forwarded on the API call).
- **Correction (root-cause owner `identity.rs`):** apply the crate's existing validated private-field newtype pattern (`TenantId`, `ObjectSha`, `InstallationToken`, `ContextId`) to the `Enterprise` payload so the field is not externally assignable; keep `enterprise()`, `api_base()`, `archive_host()` names and behaviour (they read the inner text). Only four in-crate sites name `Enterprise` (`identity.rs:319, :347, :356, :364`); no test pattern-matches the variant, so `hosts_expose_api_base_and_archive_host` and the `compile_fail` doctest survive. `#[non_exhaustive]` becomes redundant once the payload is unconstructible and may be dropped. This crate has no external consumer yet (facades are TASK-003+ in-crate), so this is not a public-API decision; if the change review disagrees, mark SPEC_REVISION_REQUIRED rather than keep the open write path.
- **Closure proof:** a second `compile_fail` doctest (or the same one extended) containing the `&mut` destructure-and-assign above; it must run under the owning lane (see FIND-TASK-002-5). `hosts_expose_api_base_and_archive_host` unchanged.

### FIND-TASK-002-5 — CONFIRMED (TASK-A-2 + TASK-B-1, same root cause)

- **Root cause:** `mise.toml:56` runs `cargo test --all-targets`, and `--all-targets` excludes the doctest target. The only executable proof of AC-R2-5 (external construction fails) is the `compile_fail` doctest at `identity.rs:310`.
- **Siblings:** `mise.toml:56` (`test:pipeline`), inherited by `test:rust` (`:58-67`) and `gate` (`:82-84`). No lane in `mise.toml` contains `--doc` (grep returned nothing).
- **Violated obligation:** AGENTS.md:40 ("An absent test target is a failing prerequisite, not a skipped pass"); `testing-workflows.md:15` ("A missing owner target fails rather than skipping"); R2 verification listed `cargo test --doc` as a focused command but never wired it into a lane.
- **Reproduction:** `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` → only `Running unittests src/lib.rs ... 16 passed`; no `Doc-tests` section. Removing `#[non_exhaustive]` would leave `test:pipeline`, `test:rust`, and `gate` green.
- **Observable consequence:** the construction seal (and any future compile-time invariant proven by doctest) has no regression guard in CI.
- **Correction (owner `mise.toml` `test:pipeline`):** reuse the existing list-form `run = [...]` already used by another task in `mise.toml` and add `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features` as a second command. (Dropping `--all-targets` would also run doctests, since the crate has no benches, but diverges from the sibling lanes.)
- **Closure proof:** `mise run test:pipeline` output shows the `Doc-tests bohmian_code_review_pipeline` section with the compile-fail test `ok`.

### FIND-TASK-002-6 — CONFIRMED (REPO-1)

- **Root cause:** unchecked arithmetic on caller-supplied bounds. `execution.rs:59` `Instant::now() + timeout` panics for `Duration::MAX` (always, all profiles). `github.rs:1273` `limit as u64 + 1` panics for `limit == usize::MAX` under overflow-checks and wraps to `take(0)` in release, returning `Ok(empty)` for a nonempty body.
- **Siblings:** `execution.rs:58-60` (public `with_timeout`); `github.rs:1271-1284` (`read_bounded`) fed by public `RequestLimits { max_json_bytes, max_blob_bytes }` (`github.rs:181-183`) via `:934`, `:1066`, `:1104`.
- **Violated obligation:** AGENTS.md:32 ("document remaining panics"); `rust-core.md:7` ("possible panics are documented or removed"); `agent-rules.md:11` (`# Panics` where relevant).
- **Reproduction (`/tmp/t002r3`):** `Execution::with_timeout(Duration::MAX)` → `should_panic ... ok`; `black_box(usize::MAX as u64) + 1` → `should_panic ... ok` (debug). Gate 3 applies as written; inputs are unrealistic, so this is a rule finding, not gate 1.
- **Correction:** `read_bounded` — stdlib `(limit as u64).saturating_add(1)` removes the panic and the release wrap. `with_timeout` — there is no representable "no deadline" `Instant`, so document `# Panics` (rule permits documented panics) rather than invent a fallback.
- **Closure proof:** rustdoc `# Panics` present on `with_timeout`; `read_bounded` has no `+ 1`; the existing `# Errors` sweep and `cargo doc` stay clean.

### FIND-TASK-002-7 — CONFIRMED (REPO-2)

- Every core/extract `mod tests` carries a rustdoc line (`core/src/diagnostic.rs:41`, `facts.rs:153`, `language.rs:306`, `edge.rs:236`, `position.rs:47`, `extract/src/buffers.rs:581`); the five new pipeline `mod tests` (`execution.rs:87`, `github.rs:1367`, `identity.rs:412`, `persistence.rs:209`, `storage.rs:184`) have none. AGENTS.md:32 covers "every new ... item, including ... tests"; a module is an item. Correction: one rustdoc line per module in the existing core wording. Proof: `grep -B1 '^mod tests' crates/bohmian-code-review-pipeline/src/*.rs` shows `///` above each.

### FIND-TASK-002-8 — CONFIRMED (REPO-3)

- `persistence.rs:97-99` uses `std::fmt::Display` / `std::fmt::Formatter` / `std::fmt::Result` path-qualified in signatures; `storage.rs:13` and `identity.rs` import `std::fmt` and use bare `fmt::…`. AGENTS.md:31 and `agent-rules.md:11` are literal. Correction: `use std::fmt;` at module top of `persistence.rs` and the same spelling as `storage.rs:126-129`. The path-qualified `serde::Deserializer` / `serde::de::Error` sites mirror `core/src/edge.rs` and `relative_path.rs`, so they are existing repo convention — not part of this finding.

## Notes (non-blocking)

- **TASK-A-1 — NOTE.** Reproduced: `serde_json::from_str::<ObjectKey>("\"x\"")` and a foreign-tenant key both `Ok` from an external crate. Not a reopen of FIND-TASK-002-1: R1's *acceptance criteria* (AC-R1-2, AC-R1-3) and R2's (AC-R2-1..3) only require refusing traversing/empty/non-key-safe segments and round-tripping constructor keys; R1 line 62's prose "foreign or traversing key is refused" was never an AC, and r2 explicitly recorded scope binding as a facade-lookup obligation. Type-level binding is impossible without identity at `Deserialize` (would need `DeserializeSeed` or a `parse(identity, raw)` API and a change to how consumers reload rows) — a public-API/persisted-data decision → SPEC_REVISION_REQUIRED if the change review wants it at the type. The zero-new-code closure for TASK-003..005: the facade already holds `Identity` and the scope, so it rebuilds the expected key with the existing `ObjectKey::snapshot` / `ObjectKey::context` and requires equality with `pointer.key` / `record.key` before any storage read; an unequal key is a persistence-implementation fault, not a read. Doc drift to tidy at that time: `storage.rs:3-5` and `:44-45` claim "never accepts an unscoped key" / "never traverse out of its scope", which today is true of the constructors only.

## Rejected

- None.

## Commands run

| Command | Result |
|---|---|
| `cat claim-ledger.md`; full read of `storage.rs`, `persistence.rs`, `identity.rs:290-380`, `execution.rs:40-75`, `github.rs:735-750, 1115-1145, 1255-1290`, R1/R2 tasks, spec lines 61/77/78/91, `mise.toml`, AGENTS.md:28-42, rust-core.md:1-12, testing-workflows.md:10-20, agent-rules.md:8-14 | read |
| `/tmp/t002r3` scratch crate (path dep on candidate crate), `cargo test` | 4 passed: TRUST-1 mutation, TASK-A-1 foreign/unscoped reload, REPO-1 two panics |
| `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` | 16 passed, no `Doc-tests` section |
| `grep -n -- '--doc' mise.toml` | no match |
| `grep -rn Enterprise crates/bohmian-code-review-pipeline/src` | 4 in-crate sites, none in tests |
| `grep -B1 '^mod tests'` across crates; `grep std::fmt::` / `serde::Deserializer` across crates | conventions as recorded above |

## Verification limits

- `read_bounded` overflow reproduced on the extracted expression, not through a fake `HttpTransport` (private fn; the expression at `github.rs:1273` is the only path). Release-profile wrap-to-`take(0)` is traced from source (`Cargo.toml` sets no `overflow-checks`), not executed.
- `mise run test:pipeline` / `gate` not re-run here; orchestrator-observed results accepted.
- No other r3 reports read.
