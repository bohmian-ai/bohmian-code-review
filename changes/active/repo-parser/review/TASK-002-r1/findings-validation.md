# TASK-002 review r1 — Wave 2 findings validation (ponytail-rev)

Subject: /tmp/task002-subject.md. Candidate dafe20b (== HEAD), base 1842eaf. Read-only; conclusions drawn from source, the claim ledger, and the authorities only (Wave 1 reports not read).

## Merge table

| Wave 1 ID | Merged claim | Reason |
|---|---|---|
| TRUST-1 | Claim 1 | — |
| REPO-1 | Claim 2 | — |
| TASK-A-1 | Claim 3/4 (merged) | Same root cause as TASK-B-1: `ObjectKind` / `ObjectKey::kind()` public surface with no production caller. TASK-A-1's second sibling (`Operation::ALL` + `CallCounts::snapshot()`) does not share the root cause (it has a real caller) and is split off and rejected below. |
| TASK-B-1 | Claim 3/4 (merged) | see above |

Claims 1 and 2 do not merge with anything: Claim 1 is a trust-boundary defect in the persisted contract; Claim 2 is a rustdoc rule gap. Claim 1's correction adds fallible items that Claim 2's closure sweep must also cover, so the closure proof for FIND-TASK-002-2 is written to run after FIND-TASK-002-1.

## Per-claim validation

### Claim 1 (TRUST-1) — CONFIRMED (blocking)

**Source trace.** `crates/bohmian-code-review-pipeline/src/storage.rs`:
- :3-5 module doc promises "The library never accepts an unscoped key: every `ObjectKey` is built here from a validated `Identity`".
- :52-54 `#[derive(... Serialize, Deserialize)] #[serde(transparent)] pub struct ObjectKey(String);` — serde mints any string as an `ObjectKey`, bypassing both constructors.
- :84-89 `pub fn context(identity: &Identity, context_id: &str)` interpolates free text after the scoped prefix; nothing rejects `/`, `.`, `..`, or empty.
- `persistence.rs:59-61` `#[serde(transparent)] pub struct ContextId(pub String);` — public field, no constructor, no validation; `:51`/`:83` `SnapshotPointer.key` / `ContextRecord.key` are `ObjectKey` and both structs derive `Deserialize`, so a consumer-persisted row reloads a key without re-validation. `Persistence::context` (:148) returns that `ContextRecord` to the library, which will read `record.key` from storage.

Contrast, same diff: `identity.rs:83-89`, `:115-122`, `:254-260` implement re-validating `Deserialize` for `TenantId`, `InstallationId`, `ObjectSha` "so persisted values are re-validated"; `RepositoryIdentity` uses `#[serde(try_from = "RawRepository")]` (:150). `ObjectKey` and `ContextId` are the only two key-bearing values in the durable seam that skip this pattern. `identity.rs:3-6` states the seam rule the crate set for itself: "Every facade and every durable seam receives these values instead of raw strings".

**Reproduction** (scratch crate /tmp/t002repro, path dep on the candidate crate, `CARGO_TARGET_DIR=<repo>/target mise exec -- cargo run --offline -q`):
```
context key: tenants/a/installations/1/repositories/7/contexts/../../../../../../tenants/b/installations/9/repositories/3/contexts/x
deserialized foreign key: Ok("tenants/b/installations/9/repositories/3/contexts/x")
context id: ../x
TenantId('../b'): Err(Tenant)
```
The last line shows the crate's existing mechanism (`is_key_safe`, identity.rs:49-52; `validate_name`, :125-135) already rejects exactly this input for tenants; it is simply not applied to the two new values.

**Gate.** Gate 3 (repository standard): AGENTS.md:29 "Use domain values such as `RelativePath` at trust boundaries"; architecture/references/languages/rust-core.md:7 "Validate canonical ... values ... at public ingress"; spec REQ-015 (spec.md:78) "The trait contract carries validated tenant, installation and repository identity ... and exact-key lookup"; REQ-014 (spec.md:77) "identities scope snapshot/context keys and every read". Consequence is gate 4-class (a reloaded or consumer-returned key naming another tenant's path is read under this tenant's identity), but the finding stands on gate 3 without needing the future read path. Not speculative hardening: the task text fixes this contract for TASK-003..006 ("Later cold/update/PR tasks consume the same source and capability contracts") and the storage.rs doc already asserts the invariant the type does not enforce. Not SPEC_REVISION-eligible: the spec already requires scoped, validated keys; the correction reuses an existing in-crate validator and changes no persisted format decision (a `ContextId` is library-minted and opaque per spec.md:37 / REQ-011, so restricting it to the key-safe alphabet it is embedded in is a library-internal choice).

### Claim 2 (REPO-1) — CONFIRMED (blocking), sibling set extended

**Verification.** `sed -n` around each of the 11 cited lines shows a one-line `///` summary and no `# Errors` section (identity.rs:125 `validate_name`, :162 `try_from`; github.rs:1075 `get_json`, :1089 `request`, :1127 `send_with_policy`, :1177 `sleep_bounded`, :1204 `expect_status`, :1217 `read_bounded`, :1232 `parse_json`, :1258 `validate_tree_entry`, :1273 `validate_pull_request_file`).

**Rule text covers private items and trait impls.** AGENTS.md:32: "Every new or materially modified Rust item, including private items and tests, requires useful rustdoc ... Fallible functions need `# Errors`". architecture/agent-rules.md:11: "New/materially changed Rust items require meaningful rustdoc, `# Errors` for fallible operations". rust-core.md:7: "Every new or materially changed item, including private helpers and tests, has intent-focused rustdoc; fallible methods explain `# Errors`".

**Precedent.** Every fallible fn in the six symbolic crates (26 sites, private and public) carries `# Errors`, including trait impls: core/relative_path.rs:83 (`Deserialize for RelativePath`), core/edge.rs:160 (`Deserialize for ReferenceKind`), core/language.rs:282 (`FromStr for Language`), extract/config.rs:51 (`TryFrom for ExtractionConfig`).

**Sibling sweep** (own rg over every `-> Result<` fn in the pipeline crate, 12-line lookback for `# Errors`): the 11 cited plus 6 more with the same root cause: identity.rs:85 (`Deserialize for TenantId`), :117 (`Deserialize for InstallationId`), :256 (`Deserialize for ObjectSha`), github.rs:132 (`HttpTransport for UreqTransport::get`), github.rs:1361 (test `FakeTransport::get`), github.rs:1397 (test `FakeRef::get`). Every public inherent method and trait declaration already has `# Errors`.

**Gate.** Gate 3, AGENTS.md:32 / agent-rules.md:11 / rust-core.md:7.

### Claim 3/4 (TASK-A-1 + TASK-B-1, merged) — CONFIRMED for `ObjectKind`/`kind()` (blocking, deletion); `CallCounts::snapshot`/`Operation::ALL` REJECTED as sibling

**Callers** (`rg -n 'ObjectKind|\.kind\(\)|Operation::ALL|CallCounts|\.snapshot\(\)' crates`):
- `ObjectKind` / `ObjectKey::kind()`: definitions storage.rs:40, :97; re-export lib.rs:32; the only uses are test asserts storage.rs:192, :198. Zero production callers. The method classifies by `self.0.contains("/contexts/")` — a substring sniff over a key whose kind is already known at every construction site.
- `Operation::ALL` (github.rs:219) is private and consumed by `CallCounts::snapshot()` (github.rs:247). `snapshot()` is consumed by the accounting test github.rs:1569 (`client.calls().snapshot()`), which is the REQ-013 / spec J2 "call accounting" evidence the task's Scenario 2 requires ("bounded request accounting"). That is a real caller for a task-required capability; it does not share Claim 3/4's root cause.

**Gate.** Gate 3 for `ObjectKind`/`kind()`: AGENTS.md:26 "Avoid generic storage/provider interfaces, future-proofing, and unnecessary features"; architecture/agent-rules.md:10 "Avoid ... speculative abstraction layers". The item is public, exported, uncalled, and encodes a heuristic that later tasks would otherwise inherit as contract. Ladder rung 1: delete. `CallCounts::snapshot`/`Operation::ALL`: no gate applies — REJECTED (not a NOTE: it is correctly consumed).

## Blocking ledger (validated)

### FIND-TASK-002-1
- Wave 1 source IDs: TRUST-1
- Status: CONFIRMED
- Gate: 3 (repository standard); consequence class 4
- Classification: trust-boundary defect in the durable contract
- Root cause: the two key-bearing values that cross the storage/persistence seam (`ObjectKey`, `ContextId`) skip the crate's own re-validating construction pattern (`identity.rs:49-52 is_key_safe`, `:125-135 validate_name`, re-validating `Deserialize` impls) and admit raw text.
- Defect class: missing validated domain value at a trust boundary.
- Sibling locations: storage.rs:52-54 (derived transparent `Deserialize` on `ObjectKey`); storage.rs:84 (`context(identity, context_id: &str)`); persistence.rs:59-61 (`ContextId(pub String)`, derived transparent `Deserialize`); persistence.rs:51 and :83 (`SnapshotPointer.key`, `ContextRecord.key` reload via derived `Deserialize`); persistence.rs:22 (`AlreadyPublished { id: ContextId }` carries the unvalidated id).
- Violated obligation: AGENTS.md:29; rust-core.md:7; spec.md:78 (REQ-015); spec.md:77 (REQ-014); the diff's own storage.rs:3-5 and identity.rs:3-6 invariants.
- Evidence: source lines above; scratch run output above.
- Reproduction: /tmp/t002repro (`cargo run` output above) — traversing context key, foreign key minted by serde, `ContextId("../x")` constructs, while `TenantId::new("../b")` is rejected by the existing validator.
- Observable consequence: an `ObjectKey` handed back by a `Persistence` implementation (or reloaded from its rows) can name another tenant's object or a traversal path, and the library reads it as this identity's object; a caller-chosen or corrupted `ContextId` can escape the `contexts/` scope. Contradicts the storage.rs:3-5 guarantee that later facades will rely on.
- Decision-complete correction (root-cause owner: `identity.rs` validators reused by `storage.rs` / `persistence.rs`; no new helper):
  1. `ContextId`: make the field private; add `pub fn new(raw: &str) -> Result<ContextId, InvalidIdentity>` that calls the existing `validate_name(raw, "context id")` (widen `validate_name` to `pub(crate)`; the 1..=100-byte `[A-Za-z0-9._-]`, not `.`/`..` rule is already what a key segment needs); replace the derived `Deserialize` with the same re-validating impl shape used by `TenantId` (identity.rs:83-89).
  2. `ObjectKey::context(identity: &Identity, context_id: &ContextId)`; `ObjectKey::snapshot` unchanged (all its parts are already validated or hex/decimal).
  3. `ObjectKey`: replace derived `Deserialize` with a re-validating impl that accepts the string only if `s.split('/').all(|seg| validate_name(seg, "object key segment").is_ok())` — the same segment rule, so a constructor-built key always round-trips and a foreign or traversing key is refused with `serde::de::Error::custom`. Do not add a new alphabet, length, or format decision; do not touch `Serialize`, `SnapshotScope`, or the trait methods.
  4. Each new fallible item gets `# Errors` (feeds FIND-TASK-002-2).
- Focused closure proof (covers every sibling): in `storage.rs` tests, `serde_json::from_str::<ObjectKey>("\"tenants/b/installations/9/repositories/3/contexts/../x\"")` is `Err` and the round-trip of a constructor-built key is `Ok(eq)`; `ObjectKey::context(&identity, &ContextId::new("ctx1")?)` equals the existing expected string; in `persistence.rs` tests, `ContextId::new("../x")` and `ContextId::new("")` are `Err(InvalidIdentity::Name(_))`, `serde_json::from_str::<ContextId>("\"a/b\"")` is `Err`, and `serde_json::from_str::<ContextRecord>` of a record whose `key` is `"tenants/b/..."` is `Err`. Run `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`; then `mise run lints` and `mise run fmt:check`.

### FIND-TASK-002-2
- Wave 1 source IDs: REPO-1
- Status: CONFIRMED (sibling set extended from 11 to 17)
- Gate: 3 (repository standard)
- Classification: mandatory rustdoc section missing
- Root cause: `# Errors` was applied only to public inherent methods and trait declarations; private fns, trait impls, and test impls were treated as exempt.
- Defect class: missing mandatory rustdoc section on new fallible items.
- Sibling locations: identity.rs:85, :117, :125, :162, :256; github.rs:132, :1075, :1089, :1127, :1177, :1204, :1217, :1232, :1258, :1273, :1361, :1397.
- Violated obligation: AGENTS.md:32; architecture/agent-rules.md:11; architecture/references/languages/rust-core.md:7.
- Evidence: `sed -n` around each site (no `# Errors`); precedent in symbolic crates (26/26 fallible fns documented, including `Deserialize`/`TryFrom`/`FromStr` impls at core/relative_path.rs:83, core/edge.rs:160, core/language.rs:282, extract/config.rs:51).
- Reproduction: the sweep command in "Verification commands" prints the 17 sites; it prints nothing for any symbolic crate.
- Observable consequence: fallible private and impl items carry no error contract, contrary to the rule every other crate follows; the crate fails the standards gate.
- Decision-complete correction: add a `# Errors` section to each of the 17 items (and to any fallible item FIND-TASK-002-1 introduces) stating the concrete variant(s) returned and when; for the serde impls, the wording already used in core/relative_path.rs:84-93 applies. No code changes.
- Focused closure proof: the sweep command below returns zero lines for the pipeline crate; `mise run lints` and `mise run fmt:check` pass.

### FIND-TASK-002-3
- Wave 1 source IDs: TASK-A-1 (ObjectKind part), TASK-B-1
- Status: CONFIRMED
- Gate: 3 (repository standard)
- Classification: unnecessary public feature
- Root cause: a key-kind classifier was added to the public contract with no production consumer.
- Defect class: speculative public API (ladder rung 1).
- Sibling locations: storage.rs:38-45 (`ObjectKind`), storage.rs:96-103 (`ObjectKey::kind`), lib.rs:32 (`ObjectKind` re-export), storage.rs:192 and :198 (test asserts).
- Violated obligation: AGENTS.md:26 ("unnecessary features"); architecture/agent-rules.md:10 ("speculative abstraction layers").
- Evidence: rg shows zero non-test callers; the only consumers are the two asserts.
- Reproduction: `rg -n 'ObjectKind|\.kind\(\)' crates --glob '*.rs'` → definitions, re-export, and two test asserts only.
- Observable consequence: public contract surface later tasks would inherit, implemented as a `contains("/contexts/")` sniff rather than data the constructor already knows.
- Decision-complete correction: delete `ObjectKind`, `ObjectKey::kind`, the lib.rs:32 re-export, and the two asserts. Nothing else changes.
- Focused closure proof: `rg -n 'ObjectKind|fn kind' crates` returns nothing; `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` passes; `mise run lints` passes.

## Notes (non-blocking)

None.

## Rejected

- TASK-A-1, second sibling (`Operation::ALL` github.rs:219, `CallCounts::snapshot()` github.rs:247): `Operation::ALL` is private and consumed by `snapshot()`; `snapshot()` is consumed by the Scenario 2 accounting test at github.rs:1569, which is the task's required "bounded request accounting" / REQ-013 evidence. Not uncalled; no gate applies.

## Verification commands and observed results

- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` → `test result: ok. 12 passed; 0 failed`.
- Scratch reproduction: `/tmp/t002repro` (`Cargo.toml` path dep on the candidate crate; `CARGO_TARGET_DIR=<repo>/target mise exec -- cargo run --offline -q`) → output quoted under Claim 1.
- Claim 2 site check: `for spec in identity.rs:125 identity.rs:162 github.rs:1075 ...; do sed -n "$((l-6)),$((l))p"; done` → each shows a one-line summary, no `# Errors`.
- Claim 2 sweep (pipeline crate): `for f in crates/bohmian-code-review-pipeline/src/*.rs; do rg -n '^\s*(pub(\([a-z]+\))? )?fn \w+[^;]*-> Result<' "$f" | while IFS=: read ln rest; do s=$((ln-12)); [ $s -lt 1 ] && s=1; sed -n "${s},${ln}p" "$f" | grep -q '# Errors' || echo "MISS $f:$ln"; done; done` plus manual read of the multi-line signatures (github.rs:1075, :1089, :1127, :1204, :1232) → 17 sites listed above.
- Precedent sweep over the symbolic crates with the same lookback → 26 HAS, 0 MISS; trait-impl fallible methods all carry `# Errors`.
- Callers: `rg -n 'ObjectKind|\.kind\(\)|Operation::ALL|CallCounts|\.snapshot\(\)' crates --glob '*.rs'` and `rg -n 'ObjectKey::context|ContextId\(|ObjectKey\(|SnapshotPointer \{|ContextRecord \{' crates --glob '*.rs'` → results as cited.
- Not run (out of budget, not required for the findings above): `mise run lints`, `mise run fmt:check`, `mise run gate`.

## Result

FAIL — three blocking findings (FIND-TASK-002-1, -2, -3).

```citations
[{"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 52, "snippet": "#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]", "symbols": ["ObjectKey"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 84, "snippet": "    pub fn context(identity: &Identity, context_id: &str) -> ObjectKey {", "symbols": ["ObjectKey::context"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 61, "snippet": "pub struct ContextId(pub String);", "symbols": ["ContextId"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 83, "snippet": "    pub key: ObjectKey,", "symbols": ["ContextRecord"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 125, "snippet": "fn validate_name(raw: &str, what: &'static str) -> Result<String, InvalidIdentity> {", "symbols": ["validate_name"]},
 {"id": "FIND-TASK-002-2", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 85, "snippet": "    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<TenantId, D::Error> {", "symbols": ["TenantId"]},
 {"id": "FIND-TASK-002-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 132, "snippet": "    fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {", "symbols": ["UreqTransport", "HttpTransport"]},
 {"id": "FIND-TASK-002-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1177, "snippet": "fn sleep_bounded(execution: &Execution, wait: Duration) -> Result<(), Interrupted> {", "symbols": ["sleep_bounded"]},
 {"id": "FIND-TASK-002-3", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 40, "snippet": "pub enum ObjectKind {", "symbols": ["ObjectKind"]},
 {"id": "FIND-TASK-002-3", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 97, "snippet": "    pub fn kind(&self) -> ObjectKind {", "symbols": ["ObjectKey::kind"]},
 {"id": "FIND-TASK-002-3", "path": "crates/bohmian-code-review-pipeline/src/lib.rs", "line": 32, "snippet": "pub use storage::{ObjectDigest, ObjectKey, ObjectKind, ObjectStorage, StorageError};", "symbols": ["ObjectKind"]}]
```
