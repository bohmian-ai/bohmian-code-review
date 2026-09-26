---
id: TASK-002-R1
kind: remediation
status: implemented
spec: SPEC-repo-parser@10
remediates: TASK-002
findings: [FIND-TASK-002-1, FIND-TASK-002-2, FIND-TASK-002-3]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Reviewed candidate: `dafe20bd6112b17aed345278d85025fe1a0aaeb2` (base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`)
- Review: `changes/active/repo-parser/review/TASK-002-r1/verdict.md`; validated ledger `findings-validation.md`

## Issue diagnosis — FIND-TASK-002-1 (unvalidated key-bearing values at the durable seam)

- **Root cause:** the two key-bearing values that cross the storage/persistence seam, `ObjectKey` and `ContextId`, skip the re-validating construction pattern the same diff applies to `TenantId`, `InstallationId`, `ObjectSha`, and `RepositoryIdentity` (`identity.rs:83-89`, `:115-122`, `:150`, `:254-260`). They admit raw text on construction and on deserialization.
- **Defect class:** missing validated domain value at a trust boundary.
- **Sibling locations (one finding):**
  - `crates/bohmian-code-review-pipeline/src/storage.rs:52-54` — derived `#[serde(transparent)] Deserialize` on `ObjectKey` mints any string as a key.
  - `crates/bohmian-code-review-pipeline/src/storage.rs:84` — `ObjectKey::context(identity: &Identity, context_id: &str)` interpolates free text after the scoped prefix.
  - `crates/bohmian-code-review-pipeline/src/persistence.rs:59-61` — `pub struct ContextId(pub String)` with a public field, no constructor, derived transparent `Deserialize`.
  - `crates/bohmian-code-review-pipeline/src/persistence.rs:51` and `:83` — `SnapshotPointer.key` / `ContextRecord.key` reload an `ObjectKey` through the derived `Deserialize`, and `Persistence::context` (`:148`) hands that record back to the library for a storage read.
  - `crates/bohmian-code-review-pipeline/src/persistence.rs:22` — `AlreadyPublished { id: ContextId }` carries the unvalidated id.
- **Violated obligation:** `AGENTS.md:29` ("Use domain values such as `RelativePath` at trust boundaries"); `architecture/references/languages/rust-core.md:7` (validate at public ingress); spec REQ-015 `spec.md:78` ("carries validated tenant, installation and repository identity ... exact-key lookup"); REQ-014 `spec.md:77` ("identities scope snapshot/context keys and every read"); the diff's own invariants at `storage.rs:3-5` ("The library never accepts an unscoped key") and `identity.rs:3-6` ("every durable seam receives these values instead of raw strings").
- **Current behavior:** `ObjectKey::context(&identity_a, "../../../../../../tenants/b/installations/9/repositories/3/contexts/x")` returns `tenants/a/installations/1/repositories/7/contexts/../../../../../../tenants/b/.../contexts/x`; `serde_json::from_str::<ObjectKey>("\"tenants/b/installations/9/repositories/3/contexts/x\"")` is `Ok`; `ContextId("../x".into())` constructs. In the same crate `TenantId::new("../b")` is `Err(InvalidIdentity::Tenant)`.
- **Evidence / reproduction:** Wave 2 scratch crate (`/tmp/t002repro`, path dependency on the candidate crate) printed exactly the four lines above; see `findings-validation.md` Claim 1. `rg -n 'ObjectKey::context|ContextId\(' crates --glob '*.rs'` shows no production caller yet; the contract is consumed by TASK-003–006.
- **Observable consequence:** an `ObjectKey` returned or reloaded by a `Persistence` implementation can name another tenant's object or a traversal path and the library reads it under this identity; a caller-chosen or corrupted `ContextId` escapes the `contexts/` scope. The guarantee `storage.rs:3-5` advertises to consumers is false at the type level, so every later facade would have to re-implement the check.
- **Why the candidate falls short:** the existing tests (`storage::tests::keys_are_scoped_by_identity_and_version`) only exercise well-formed inputs through the constructors; nothing exercises deserialization or a hostile `context_id`.

## Issue diagnosis — FIND-TASK-002-2 (missing `# Errors` on fallible items)

- **Root cause:** `# Errors` was written only for public inherent methods and trait declarations; private functions, `Deserialize`/`TryFrom`/`HttpTransport` impls, and test fakes were treated as exempt.
- **Defect class:** missing mandatory rustdoc section on new fallible items.
- **Sibling locations (17, one finding):** `identity.rs:85` (`Deserialize for TenantId`), `:117` (`Deserialize for InstallationId`), `:125` (`validate_name`), `:162` (`TryFrom<RawRepository>`), `:256` (`Deserialize for ObjectSha`); `github.rs:132` (`HttpTransport for UreqTransport::get`), `:1075` (`get_json`), `:1089` (`request`), `:1127` (`send_with_policy`), `:1177` (`sleep_bounded`), `:1204` (`expect_status`), `:1217` (`read_bounded`), `:1232` (`parse_json`), `:1258` (`validate_tree_entry`), `:1273` (`validate_pull_request_file`), `:1361` (test `FakeTransport::get`), `:1397` (test `FakeRef::get`). Plus every fallible item FIND-TASK-002-1 introduces.
- **Violated obligation:** `AGENTS.md:32` ("Every new or materially modified Rust item, including private items and tests, requires useful rustdoc ... Fallible functions need `# Errors`"); `architecture/agent-rules.md:11`; `architecture/references/languages/rust-core.md:7` ("including private helpers and tests ... fallible methods explain `# Errors`").
- **Current behavior / evidence:** each site carries a one-line `///` summary and no `# Errors` section (`sed -n` around each line). Precedent: all 26 fallible functions in the six symbolic crates carry the section, including trait impls (`core/relative_path.rs:83`, `core/edge.rs:160`, `core/language.rs:282`, `extract/config.rs:51`).
- **Reproduction:** the sweep in `findings-validation.md` ("Verification commands") prints the 17 sites for the pipeline crate and nothing for any symbolic crate.
- **Observable consequence:** the single request policy (`send_with_policy`) and the seam validators carry no documented error contract, contrary to the rule every other crate follows. Not caught by `lints` (`missing_errors_doc` is pedantic and public-only).

## Issue diagnosis — FIND-TASK-002-3 (uncalled public key-kind classifier)

- **Root cause:** a key-kind classifier was added to the public contract with no production consumer; the kind is already known at every construction site.
- **Defect class:** speculative public API (ladder rung 1).
- **Sibling locations (one finding):** `storage.rs:38-45` (`ObjectKind`), `storage.rs:96-103` (`ObjectKey::kind`, implemented as `self.0.contains("/contexts/")`), `lib.rs:32` (`ObjectKind` re-export), `storage.rs:192` and `:198` (the only two uses, test asserts).
- **Violated obligation:** `AGENTS.md:26` ("Avoid generic storage/provider interfaces, future-proofing, and unnecessary features"); `architecture/agent-rules.md:10` ("speculative abstraction layers").
- **Evidence / reproduction:** `rg -n 'ObjectKind|\.kind\(\)' crates --glob '*.rs'` → definitions, re-export, and the two test asserts only.
- **Observable consequence:** later tasks inherit a public heuristic that silently returns `Snapshot` for any non-context key.

## Intended correction outcome

Every value that crosses the storage/persistence seam is constructible only through validation and re-validates on deserialization, using the validators the crate already has; every fallible item in the crate documents `# Errors`; the uncalled classifier is gone. Provider client behavior, trait method sets, key string layout, `Serialize` output, and every existing test expectation are unchanged.

## Decision-complete recommendation

**FIND-TASK-002-1** — reuse the existing `identity.rs` validators; add no new alphabet, length, or format decision.

1. `ContextId` (`persistence.rs`): make the field private and add a fallible `ContextId::new(&str)` that validates through the existing `validate_name` (`identity.rs:125`, widened to `pub(crate)`; its 1..=100-byte `[A-Za-z0-9._-]`, not-`.`/`..` rule is exactly a key segment). Replace the derived `Deserialize` with the re-validating impl shape `TenantId` uses (`identity.rs:83-89`). Keep `Serialize`, `Display`/`Debug`, and `AsRef<str>` as they are so persisted output does not change. A `ContextId` is library-minted and opaque (spec.md:37, REQ-011), so restricting its alphabet is a library-internal choice, not a spec change.
2. `ObjectKey::context` takes `&ContextId` instead of `&str`. `ObjectKey::snapshot` is unchanged (its parts are already validated identity, hex, and decimal).
3. `ObjectKey` (`storage.rs`): replace the derived `Deserialize` with a re-validating impl that accepts the string only when every `/`-separated segment passes `validate_name`; a constructor-built key therefore round-trips and a foreign or traversing key is refused via `serde::de::Error::custom`. `Serialize`, `SnapshotScope`, `SnapshotPointer`, `ContextRecord`, and the trait methods are untouched; they inherit the fix through the field type.

Why this closes the class: the three admission points (constructor argument, `ObjectKey` deserialization, `ContextId` construction/deserialization) all route through one existing validator, so `SnapshotPointer.key`, `ContextRecord.key`, and `AlreadyPublished.id` are covered without touching them. Rejected alternatives: validating inside each future facade (repeats the check at every consumer, contradicts `storage.rs:3-5`); storing key components instead of the string (changes the persisted format, a decision the spec does not require).

**FIND-TASK-002-2** — add an `# Errors` section to each of the 17 items and to every fallible item introduced by the correction above, naming the concrete variant(s) and when they are returned (for `send_with_policy`: `Interrupted` via `Execution::check`, `RateLimited`, `Forbidden`, `Unavailable`, `Transport`, and the terminal status errors). For the serde impls, the wording in `core/relative_path.rs:84-93` applies. Documentation only; no code changes.

**FIND-TASK-002-3** — delete `ObjectKind`, `ObjectKey::kind`, the `lib.rs:32` re-export, and the two test asserts. Nothing else changes.

## Constraints, preserved behavior, and non-goals

- Do not change the key string layout, `Serialize` output, trait method sets, provider client behavior, request policy, error enums beyond what `ContextId::new` needs, or any existing test expectation.
- Do not add a dependency, a new validator, a helper struct, or a trait.
- Do not add facade code, an OpenDAL adapter, in-memory trait conformance implementations (TASK-006), a concurrency limiter, or a real-GitHub run.
- Do not alter the six symbolic crates, grammar pins, or fixtures.
- Keep tests inline in the owning module.

## Acceptance criteria

- **AC-R1-1 (FIND-TASK-002-1, `storage.rs:84`):** `ObjectKey::context` accepts only a `&ContextId`; with `ContextId::new("ctx1")` it produces the string the existing test already expects.
- **AC-R1-2 (FIND-TASK-002-1, `storage.rs:52-54`, `persistence.rs:51`, `:83`):** deserializing `"tenants/b/installations/9/repositories/3/contexts/../x"` (and any string with an empty, `.`, `..`, or non-key-safe segment) as `ObjectKey`, `SnapshotPointer`, or `ContextRecord` is `Err`; a constructor-built key round-trips through serde equal to itself.
- **AC-R1-3 (FIND-TASK-002-1, `persistence.rs:59-61`, `:22`):** `ContextId::new("../x")` and `ContextId::new("")` are `Err(InvalidIdentity::Name(_))` (or the existing variant `validate_name` returns for the given `what`); `serde_json::from_str::<ContextId>("\"a/b\"")` is `Err`; the field is not publicly constructible.
- **AC-R1-4 (FIND-TASK-002-2, all 17 sites + new fallible items):** the sweep command in `findings-validation.md` prints no `MISS` line for `crates/bohmian-code-review-pipeline`.
- **AC-R1-5 (FIND-TASK-002-3):** `rg -n 'ObjectKind|fn kind' crates` returns nothing.
- **AC-R1-6:** no other file changes beyond `storage.rs`, `persistence.rs`, `identity.rs`, `lib.rs`, and the task document's evidence section.

## Focused proof and broader verification

- Focused: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests -- --exact` and `... persistence::tests ...` (new inline tests covering AC-R1-1..3); run the `# Errors` sweep from `findings-validation.md`; run the `rg` in AC-R1-5.
- Broader: `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`; `git diff --stat` shows only the files in AC-R1-6.
- Record observed results in the task document's evidence section per `architecture/references/languages/implementation-execution.md`.

Route to `$cr-implement`. A later review reassesses the complete cumulative candidate against the original TASK-002.

## Implementation evidence

Implemented in commits `30b5191` and `5a8cf76`; the AC-R1-1..6 evidence table, excluded non-goals, and the full command list are recorded under "TASK-002-R1 remediation evidence" in `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`. All verification commands exited 0; `mise run gate` passed.
