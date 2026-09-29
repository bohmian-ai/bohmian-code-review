# bohmian-code-review contributor rules

This repository owns reusable Rust helpers for in-memory symbolic code-review context. `architecture/bohmian-code-review-design.md` is the design authority. `changes/active/` holds approved change specifications and task evidence. Wyrd and codegraph-rs are reference repositories, not runtime dependencies or authorities for this repository.

## First pass before editing

1. Read this file and `architecture/agent-rules.md`.
2. Read `architecture/bohmian-code-review-design.md` and route through `architecture/references/README.md` to only the relevant references.
3. Identify the owning crate, its consumers, and the nearest tests. Check `mise.toml`, the workspace manifests, and `Cargo.lock` before choosing verification or relying on dependency behavior.
4. If a `.codegraph/` index exists at the repository root, use CodeGraph before text search or file reads. Otherwise use targeted `rg` searches.

## Product and ownership boundaries

- `bohmian-code-review-core` owns IO-free shared facts and validated domain values.
- `bohmian-code-review-extract` owns tree-sitter routing, native and composite extraction, grammar pins, and bounded synchronous batch parsing.
- `bohmian-code-review-index` owns in-memory symbol-to-file postings and replacement/removal.
- `bohmian-code-review-resolve` owns candidate-file lookup from extracted reference names; ambiguous matches stay ambiguous.
- `bohmian-code-review-select` owns the `Ranker` trait and its implementations (symbol overlap, rarity-weighted symbols, co-change, reciprocal-rank fusion), deterministic ranking, and bounded context packing.
- `bohmian-code-review` owns the public two-pass facade and no duplicate stage logic.

These six symbolic crates accept caller-supplied repository-relative paths, source bytes, changed-line ranges, and token counts. They do not clone, discover Git roots, read source from disk, make network calls, persist indexes or source, invoke models, or publish review comments. They may export and reload source-free declaration facts, expose the extraction version those facts were produced under, and answer path-only eligibility queries, but never retain or persist source text themselves.

The approved outer pipeline library crate (`changes/active/repo-parser/spec.md`) is the only crate permitted Git provider and storage IO. It owns GitHub/GHES source access, Git identity and blob verification, repository eligibility, bounded ingest, snapshot build/update, PR overlays, context publication and read-back, and the narrow storage/persistence traits a consumer implements. It depends on the public `bohmian-code-review` facade only and must not duplicate extraction, indexing, resolution, scoring, or packing. The feature-gated `bohmian-code-review-eval` crate evaluates rankers through the public `Ranker` trait and reads the provider only through pipeline types; it never ranks on its own and is never a runtime dependency. A consuming service owns credentials, webhooks, scheduling, review agents, models, comment publication, and cross-repository search. Never report a partial caller-supplied index as complete repository coverage.

## Rust and design rules

- Keep stateful capabilities and workflows on cohesive concrete structs. Use free functions only for stateless transformations; use traits only for multiple real implementations. Avoid generic storage/provider interfaces, future-proofing, and unnecessary features or dependencies.
- Use `&str`, `&[u8]`, slices, and references when ownership is unnecessary. Treat `.clone()` as an ownership decision. Use domain values such as `RelativePath` at trust boundaries.
- Durable time comes from the persistence store's clock (the database clock), never the host clock. Do not pass host timestamps across store traits.
- Use `thiserror` for library errors, `tracing` with structured fields when diagnostics are needed, and no unchecked `unwrap()` for input, environment, parsing, or IO. `expect()` is reserved for named invariants.
- Keep imports at module top and use imported bare types in signatures.
- Every new or materially modified Rust item, including private items and tests, requires useful rustdoc describing intent, workflow, and invariants. Fallible functions need `# Errors`; document remaining panics and cancellation/partial progress where applicable.
- Keep grammar pins and checked-in extraction fixtures intact. Do not alter expected output to make a port pass. Do not hand-edit generated artifacts.
- No production Node, NAPI, SQLite, PostgreSQL, PyO3, Git provider SDK, model SDK, or remote source dependency. Vendored grammar C is build-time input.

## Tests and verification

The primary contract is a public-library journey using real extractor → index → resolver → selector code on in-memory source. It proves the two-pass file picker, ranking, and token budget without a server, database, credentials, network, or local checkout. Component integration and unit tests support that journey; neither substitutes for it.

Use `mise.toml` task names. Run the narrowest owner lane plus `fmt:check` and `lints` for a local change; run `test:symbolic-context:journey` when behavior crosses crate boundaries. Run `gate` for broad changes or release candidates. An absent test target is a failing prerequisite, not a skipped pass. Never weaken a gate, suppress a real lint, delete a failing test, or rewrite a golden to produce green output.

## Change lifecycle and Git

New work follows the repository's `.agents/skills/cr-*` lifecycle: approved spec, bounded tasks, implementation, independent task review, integrated change review, and completion record. The current user instruction takes precedence over repository skills. Keep plan/task/agent references out of production code.

Use the locally configured Git identity. Do not change identity, forge another author, or add AI co-author trailers. Push, tag, or publish only when the user has authorized that action; never infer publication from a passing gate.
