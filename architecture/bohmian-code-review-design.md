# bohmian-code-review design

## Purpose

This Rust workspace builds symbolic context for code review from **caller-supplied, in-memory source**. Six symbolic crates form an IO-free library; an outer pipeline library crate, approved in `changes/active/repo-parser/spec.md`, gets Git tree diffs and blobs through provider APIs, verifies repository/commit/blob identity, and passes bounded source into the symbolic facade. Neither clones or holds a working copy. Runtime source bytes are transient: the index retains extracted facts, exported facts are source-free, and the returned context owns only selected text.

The first required user journey is CodePal's two-pass file picker. Pass one parses supplied files with tree-sitter and builds a symbol-to-file index. Pass two parses changed head-side files, gathers declarations overlapping changed lines and references located on changed lines, resolves those names to plausible index hits, ranks other files by distinct-symbol overlap, and packs relevant spans under caller-supplied file and token limits. A shared context store, review loop, verifier, and cross-repository Code Search belong to the service or a separate service.

## Crate direction

| Crate | Durable owner |
|---|---|
| `bohmian-code-review-core` | IO-free facts, coordinates, validated paths, and boundary values |
| `bohmian-code-review-extract` | Full native/composite tree-sitter extraction and grammar resources |
| `bohmian-code-review-index` | In-memory symbol postings and per-file replacement/removal |
| `bohmian-code-review-resolve` | Candidate matching from extracted reference names to index hits |
| `bohmian-code-review-select` | Scoring, ranking, and budgeted source selection |
| `bohmian-code-review` | Public facade composing the stages |

Dependencies point only downward: core → extract/index → resolve → select → facade. `index` consumes core facts rather than depending on a parser. `resolve` returns candidates, not a semantically proven target. Score and top-file picking share one owner because their ordering and budget decisions form one contract. The facade composes owners and must not recreate their algorithms.

The outer pipeline library crate sits above the facade and is the only crate with Git provider or storage IO. It owns GitHub/GHES access, identity and blob verification, eligibility, bounded ingest, source-free snapshots, PR overlays, ranked context publication, read-back by context ID, and the narrow storage/persistence traits a consumer implements. It depends on `bohmian-code-review` alone, never on a consuming crate, and starts no server or worker loop. The six symbolic crates gain no IO from it.

## Public behavior

Paths are canonical repository-relative UTF-8 paths. Source coordinates exposed to callers are one-based lines. The caller supplies full head-side changed-file bytes plus nonempty ascending changed-line ranges. Extraction errors are typed and file-local where the extractor permits recovery. Index replacement removes old postings before new facts become visible; removal is idempotent. Exact simple and qualified symbol lookup is case-sensitive and returns all ambiguous hits in canonical path/location/ID order.

The candidate resolver matches exact extracted `reference_name`, extractor-provided qualified candidates, and changed declaration names to symbol postings. The selector excludes the changed file from secondary candidates. Each distinct matching name contributes one point per candidate file, regardless of repeated occurrences. Candidates sort by descending score, then ascending canonical path. No fuzzy matching or exact-definition claim is made in v1.

The caller supplies selected candidate blobs after ranking and supplies `Fn(&str) -> usize` for model-specific token counting. Context packing includes changed hunks first, then ranked relevant symbol spans, subject to `max_files` and `max_tokens`. It returns selected text with path, line range, score, matched names, token count, and explicit omissions. An unfittable changed hunk is a typed budget error; a missing candidate blob is an explicit input error. Source is never silently cut mid-span. Deletion-only changes are explicitly unsupported in the initial release.

An index describes only the files its caller supplied. The symbolic library has no repository discovery or completeness authority. The outer pipeline crate decides input coverage and owns Git provider access, commit/blob verification, and shared context storage; a consuming service owns model review, feedback, and cross-repository discovery.

To serve that pipeline, the facade exports each indexed file's byte length and declaration facts without docstrings, signatures, or other source fragments, and loads such facts into a fresh index so lookup, ranking, and candidate spans match a fresh parse. Every exported record is stamped with the extraction version; loading a record from another version or with malformed facts fails explicitly and leaves other files intact. A path-only language query reports whether a path would be extracted before its source is downloaded; ambiguous routes may still use content at parse time. Multi-file ranking unions the changed-line names of every changed head blob, excludes every changed path, and scores once per distinct name; candidate-only packing accepts that ranking without a changed head blob and counts only selected candidate files.

## Verification contract

The primary journey drives the public facade from in-memory TypeScript files through both passes, candidate ranking, and exact caller-reported token accounting. Component tests cover each crate independently. Extractor fixtures are copied from the donor repository with grammar pins and provenance intact; tests must not read the donor checkout. Mise lanes are scoped by crate and journey, with a full gate for broad changes. No benchmark claim is accepted without a named environment and reproducible measurements.
