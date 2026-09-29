# Domain review: source trust and live-report provenance

Subject: base `731a4be1ca0e2bc9c3f44526d26d99e41c487d88`, candidate `989f212b4f747cde51d37bde15737a7f5fc893a5`.

## Boundary and coverage

I reviewed the complete base-to-candidate diff and the source path from `Api::get` and `Api::select` through `Discovery::offer`/`into_sample`, `Api::listing`, `Listing::push`, `Screen::new`/`qualifies`, `ColdBuild::run_at`, `Api::trials`, and JSON publication. I also inspected the provider's `pull_request_files` and hash-verified blob path, the source-free report types, both complete JSON artifacts, and the opt-in mise lane. Governing authority: approved spec revision 4 (`REQ-001`, `REQ-002`, `REQ-006`, `REQ-007`, `INV-001`, `INV-003`–`INV-005`, `AC-001`, `AC-002`, `AC-005`), TASK-003, `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the spec-driven and testing references.

| Boundary | Evidence | Result |
|---|---|---|
| PR discovery and qualification | `main.rs:287–329` reads all creation-ascending pages before qualification; `sample.rs:159–202` rejects duplicate numbers and unusable times/commits for kept merged PRs, then orders by merge time and number. `main.rs:116–217` consumes one paginated file listing and a fixed-commit complete base snapshot per plausible PR. | PASS |
| Pinned source and output | `main.rs:227–276` fetches base and changed-head blobs by object SHA through the pipeline client; `main.rs:351–399` emits report fields after trials complete. The two JSON reports each have 50 distinct, correctly ordered PRs and 40-character base, head, and tree IDs; all 330 trials reference a sampled PR and source path. | PASS |
| Report integrity and source lifetime | The report schema carries paths, names, IDs, scores, counts, and fixed rule text. `DiffError` omission messages carry a line number and category, not patch contents. Report inspection found no credential or source-text fields; `mise.toml:76–78` keeps the live command outside `gate`. | PASS |

## Verification and limits

I parsed both checked-in reports: 50 unique ordered PRs each, 171 and 159 trials respectively, valid commit shapes, every trial linked to its sample, summary trial counts equal the trial arrays, and each variant's miss list equals its unranked or omitted trials. Secret-pattern search over both artifacts found no `GITHUB_TOKEN`, GitHub token prefix, bearer header, private-key marker, or source-text field. `git diff --check` exited 0, and HEAD remained the candidate commit. I did not repeat the two credentialed, roughly 20-minute runs; TASK-003 records two byte-identical full-run outputs and their SHA-256, but the original stdout files are not retained. Live provider behavior and the rerun comparison therefore rest on that recorded execution evidence. A PR deleted during discovery could still shift unread pages without a duplicate, as `tasks/TASK-003-live-reports.md:87` records; I found no new material source-trust issue introduced by this candidate.

## Material findings

None.

**Overall: PASS.**

```citations
[]
```
