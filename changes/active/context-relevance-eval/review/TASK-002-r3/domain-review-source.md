# Source trust and parser provenance review

**Subject:** base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `012aa47dbf2b7885d1e9ba25eb1b22c861113629`. **Result: FAIL.**

## Boundary, authority, and coverage

| Boundary | Authority | Source inspected | Result |
|---|---|---|---|
| Complete indexed base corpus and verified blobs | Spec REQ-004, INV-001–002, INV-005; original TASK-002 scenarios 1–2; `AGENTS.md` outer-pipeline ownership | `main.rs:220–276` obtains every indexed snapshot path and its tree blob ID, then calls `GitHubClient::blob`; `trial.rs:225–277` requires every indexed base source and extracts references through `SymbolicContext`. `github.rs:1237–1269` bounds and verifies Git blob bytes. | PASS |
| Head source and parser provenance | Spec REQ-004, INV-002–003; TASK-002 scenario 2 | `main.rs:242–275` takes head blob IDs from the one PR listing and fetches verified bytes. `trial.rs:335–348,529–626` uses the public facade for head analysis. Failure handling at `trial.rs:590–596` is defective, as below. | FAIL |
| D import provenance and resolved scope | Spec D rule at `spec.md:32`; TASK-002-R1 and R2 | `trial.rs:366–487` derives bindings only from public Python/Rust import references and nodes. Both base and head call the shared binding owner (`:267`, `:618–623`). `score` keeps a uniquely resolved module as scope even if it lacks the name (`:743–761`), preserves the complete declaring-file denominator, and falls back to C for unresolved bindings. Public producers inspected: `python.rs:620–715`, `rustlang.rs:825–959`. | PASS |
| Source lifetime and report serialization | Spec INV-004–005 | `main.rs:351–402` retains fetched bytes in a command-local cache and serializes `Trial` plus `report` summaries/misses. `trial.rs:85–149` and `report.rs:15–41` contain paths, names, scores, and counts, with no source field. | PASS |

## Finding

**SOURCE-1 — Incorrect failure classification: a valid patch whose changed lines do not fit its verified head becomes a successful omission.** Classification: **INCORRECT / source-trust boundary**. The owner is `Change::new` at `trial.rs:590–596`; its shape is not the defect, but its catch-all `Err` arm is. It converts *every* public-facade ranking or extraction failure to `Ok(change.omit(...))`. `Screen::new` already distinguishes missing or malformed patches; a syntactically valid hunk with head line 99 against a one-line verified head passes that screen, while `ChangedLines::new` rejects it as `RangeBeyondSource`. `Change::new` then erases that file's names and head, so `rank_a` skips it and B–D also receive no names, yet `evaluate` returns trials and the command prints a successful report. This violates spec REQ-004 (remaining changes supply head changed-line names) and INV-003 (provider inconsistency fails the run); only missing or malformed *patches* are authorized omissions by REQ-002/REQ-005. The same root cause covers every changed Python/Rust head with a parsed patch that fails facade validation, UTF-8 decoding, or extraction. The sibling effects are A's `rank_a` head filter at `trial.rs:833–845`, B–D's `Query::new` at `:687–703`, and report success at `main.rs:351–402`; base extraction and missing head/blob paths already fail explicitly and need no separate correction.

**Correction:** keep omission for missing/malformed patches, but propagate public-facade analysis errors from `Change::new` as `TrialError::Symbolic`, so a parsed patch inconsistent with its verified head fails the run. A focused no-network check should give a valid hunk whose head range exceeds the verified head and assert `evaluate` returns that error instead of trials.

## Verification and limits

The cumulative base-to-candidate diff, original task, both remediation tasks, relevant design and symbolic-context authorities, snapshot owner, GitHub blob verifier, and public Python/Rust import producers were inspected. `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` passed 6 tests; `git diff --check <base> <candidate>` passed. The focused suite has no parsed-patch/head-mismatch case. No live credentialed GitHub run was performed; TASK-003 owns it. The finding follows the existing parsed-hunk and facade validation control flow; no subject source was changed.

```citations
[{"id":"SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":596,"snippet":"            Err(error) => return Ok(change.omit(format!(\"analysis failed: {error}\"))),","symbols":["Change::new","TrialError::Symbolic"]},{"id":"SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":835,"snippet":"        .filter_map(|change| {","symbols":["rank_a","Change::new"]}]
```
