# Repository standards review

Subject: base `322a2b9e0984062625d54274798a9c32cb01142d` through immutable candidate `efc381f776e397ace4f43f9f4ab45fc4187436a5`. This is the complete cumulative diff, including R1 remediation. No source was edited. The untracked `changes/active/impact-map/` was left alone. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority | Result |
|---|---|---|
| Core facts; index postings; resolver; selector rankers and packing; symbolic facade and public journey | `AGENTS.md:14-21,25-40`; `architecture/agent-rules.md:3-12`; `architecture/bohmian-code-review-design.md:5-36`; `architecture/references/languages/rust-core.md:3-7`; `architecture/references/languages/testing-workflows.md:3-19`; `architecture/references/domain/symbolic-context.md:3-13` | PASS |
| Pipeline provider, history, snapshots, updates, PR context, consumer wrapper, stores, and fake-provider tests | `AGENTS.md:23,25-40`; `architecture/agent-rules.md:3,5,7-12`; `architecture/bohmian-code-review-design.md:5-36`; Rust core and testing workflows above; `architecture/references/domain/pipeline-consumer-ergonomics.md:3-203` | PASS |
| Evaluation crate, live binary, trial and report libraries, feature and test lane | `AGENTS.md:23,25-40`; `architecture/agent-rules.md:3,5,10-12`; `architecture/bohmian-code-review-design.md:24,38`; Rust core and testing workflows above | FAIL: STANDARD-1 |
| Architecture, router, README, manifests, lockfile, mise tasks, review records | `AGENTS.md:3,7-9,38-46`; `architecture/agent-rules.md:14-15`; `architecture/references/README.md:3-14`; `architecture/references/languages/spec-driven-development.md` and `implementation-execution.md` | PASS; the user expressly substitutes approved `intent.md` for an absent task file |
| Grammar pins, fixtures, generated files | `AGENTS.md:33-34`; `architecture/agent-rules.md:6,12`; testing workflows | PASS: no extractor, fixture, or generated artifact changed |

## Rule results and evidence

| Written rule | Result | Source and check evidence |
|---|---|---|
| Symbolic ownership and one-way dependency, IO-free facts, pipeline-only Git/storage IO (`AGENTS.md:14-23`; `agent-rules.md:3,9`) | PASS | Symbolic changes stay in `core`, `index`, `resolve`, `select`, and facade; `pipeline/src/github.rs` owns provider reads; eval `live.rs` calls `GitHubClient`; eval manifest depends on facade and pipeline only. |
| Concrete workflow owners, real trait seams, borrowed inputs (`AGENTS.md:27-28`; `agent-rules.md:10`; Rust core:3-5) | PASS | `ReviewPipeline` owns workflow, `Ranker` has multiple implementations, `HttpTransport` has production and fake implementations, and store traits are consumer seams. No introduced source retention in the index. |
| Canonical input and changed-range validation, complete replacement, explicit omissions (`agent-rules.md:7-9`; symbolic context:3-13) | PASS | Diff keeps `RelativePath` and `ChangedLines` ingress, index replacement/load validation, and explicit packing failures; public and component tests cover them. |
| Library errors use `thiserror` typed errors (`AGENTS.md:30`; Rust core:7) | **FAIL** | Newly public eval library live workflow returns erased `Box<dyn Error>` from all fallible entry points and internal siblings; see STANDARD-1. |
| Structured diagnostics, no unchecked input/IO panics, imports and rustdoc (`AGENTS.md:30-32`; `agent-rules.md:11`) | PASS otherwise | Added `expect` sites are fixed binary constants or tests; live and pipeline functions document errors. The erased error defect is counted once above. |
| Store-clock durable time and source lifetime (`AGENTS.md:21,29`; `agent-rules.md:9`; design:5,20) | PASS | New history stores provider merge times; `SystemTime::now` in `github.rs` is a non-durable request timer. Store traits retain the database-clock contract. |
| Public journey and test placement, no weakened gate (`AGENTS.md:38-40`; `agent-rules.md:4-5,12`; testing workflows:3-19) | PASS | Modified `symbolic_context_journey.rs` and `pipeline_journey.rs` cross facade boundaries; eval tests are inline; `mise.toml` adds `test:eval` to `test:rust`. `git diff --check` passed. R1 implementation record reports exit 0 for `test:eval`, `test:pipeline`, `test:pipeline:journey`, `fmt:check`, `lints`, and `gate`; these runs were not independently repeated in this standards review. |
| No prohibited dependency, grammar or generated-output edits; no publication (`AGENTS.md:33-34,46`; `agent-rules.md:6,13,15`) | PASS | Manifest adds the feature-gated eval crate using existing workspace dependencies. No grammar/fixture/mirror edits, push, or tag in the candidate. |

## Material finding

### STANDARD-1 — Erased error type at the newly public eval library boundary

**Classification:** VIOLATION. **Defect class:** fallible library API returns `Box<dyn Error>` instead of a typed `thiserror` error. **Root cause and shape:** the former executable's live workflow was promoted to exported library functions while retaining its generic error return type. `Api::select` also creates an untyped string error from a malformed repository name. The public shape permits callers to inspect errors only by downcasting or message text. **Written rule:** `AGENTS.md:30` says “Use `thiserror` for library errors”; `architecture/references/languages/rust-core.md:7` says “Use typed library errors with `thiserror`.”

**Owner:** `crates/bohmian-code-review-eval/src/live.rs:340` (`live`). **Sibling sweep:** `Api::base_snapshot:118`, `Api::qualify:160`, `Api::trials:214`, `Api::select:263`, public `run:360`, and public `print:487` share the erased return type. `Api::select:267` injects a bare string error. The typed `TrialError`, `SampleError`, `ArmsError`, and pipeline errors show the existing error categories available for a small wrapping enum; no other changed crate showed this pattern.

**Consequence:** consumers of the reusable evaluation library cannot reliably branch on missing token, invalid repository, incomplete sample, provider retry state, or trial failure through its public API. **Testable correction:** give the live workflow one `thiserror` error enum with variants for its existing error categories and explicit malformed repository/token failures; propagate it through the public functions and private siblings. Add one caller test that matches a representative variant without downcasting. Keep the binary's final display behavior.

**Overall: FAIL.** This is one introduced repository-rule violation. No task-acceptance judgment or optional improvement is included.

```citations
[{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":118,"snippet":"    ) -> Result<Snapshot, Box<dyn Error>> {","symbols":["Api::base_snapshot"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":160,"snippet":"    ) -> Result<Option<Sampled>, Box<dyn Error>> {","symbols":["Api::qualify"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":214,"snippet":"    ) -> Result<(), Box<dyn Error>> {","symbols":["Api::trials"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":263,"snippet":"    async fn select(&self, repo: &Repository) -> Result<Selected, Box<dyn Error>> {","symbols":["Api::select"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":267,"snippet":"            .ok_or(\"repository must be <owner>/<name>\")?;","symbols":["Api::select"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":340,"snippet":"pub fn live() -> Result<Api, Box<dyn Error>> {","symbols":["live"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":360,"snippet":") -> Result<Vec<Value>, Box<dyn Error>> {","symbols":["run"]},{"id":"STANDARD-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":487,"snippet":"pub async fn print(repos: &[Repository], arms: Arms<'_>) -> Result<(), Box<dyn Error>> {","symbols":["print"]}]
```
