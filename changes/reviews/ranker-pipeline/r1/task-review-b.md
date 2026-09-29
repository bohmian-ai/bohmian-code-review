# Task implementation review B

Subject: `322a2b9e0984062625d54274798a9c32cb01142d` → `100249c3031d645ce8db9148b4b6fa43e71481d1`. Authority: `changes/reviews/ranker-pipeline/intent.md` (no task file, per maintainer), `architecture/references/domain/pipeline-consumer-ergonomics.md`, and repository rules. Reviewed the complete diff and the relevant public callers. The orchestrator reports `mise run gate` and `git diff --check` passed at the candidate.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public, IO-free `Ranker` with S/C/E/F and experiment semantics | `select/src/rank.rs:25-36,106-319`; facade re-export in `bohmian-code-review/src/lib.rs:28-31` | Ranker owner tests; public journey; gate | PASS |
| 2. F default, exact C order without history | `select/src/rank.rs:25-27,294-319`; default facade calls | `rank.rs:630-695`; gate | PASS |
| 3. Feature-only reusable evaluation taking a sample **and named rankers**, report and live binary | Eval feature and moved binary exist; `trial.rs:37-42,273-278,297-303` fixes arms globally; `live.rs:473-485` accepts repositories only | Eval tests and gate prove fixed S/C/E/F path, not injected arms | FAIL |
| 4. Persist distinct reference and candidate names; snapshot v4 | `index/src/lib.rs:38-50,85-120,217-225`; `snapshot.rs:38` | Index round-trip tests; gate | PASS |
| 5. Fractional score; context v2 | `select/src/lib.rs:121-131`; `pipeline/src/context.rs:29` | Journey and gate | PASS |
| 6. Pack declaration spans for co-change-only candidates within budgets | `select/src/lib.rs:374-460` | Public journey `co_change_history_ranks_and_packs_a_file_sharing_no_symbol`; gate | PASS |
| 7. Bounded persisted history in both passes, F ranking, time and self exclusion | `pipeline/src/history.rs:319-432`, `snapshot.rs:723-752`, `pull_request.rs:222-229,448-463,619-658` | History, PR context, and pipeline journey tests; gate | PASS |
| 8. One `ReviewPipeline` over shared storage/store/limits/error; cold build, refresh, read; public stage facades | `pipeline/src/review.rs:27-155`; `pipeline/src/lib.rs`; README | `pipeline_journey.rs:618` and gate | PASS |
| Evaluation provider reads only through pipeline types | `eval/src/live.rs:145-190,238-257,372-392` owns a direct REST client alongside `GitHubClient` | Source trace; gate cannot establish this architecture constraint | FAIL |
| No runtime eval dependency or source persistence; no background work or algorithm config | Feature-gated eval manifest and code; pipeline and symbolic crate boundaries | Manifest inspection; gate | PASS |

## Proposed findings

### TASK-B-1 — MISSING: evaluation arms are fixed inside the harness

- **Root cause / defect class:** `trial::ARMS` is the only arm set and `trial::evaluate` has no named-ranker argument. The shape of the public evaluation API makes the required ranker input unrepresentable.
- **Violated obligation:** Decision 3 in `intent.md:9`: the reusable harness takes a repository sample and named `&dyn Ranker`s.
- **Owner and siblings:** `crates/bohmian-code-review-eval/src/trial.rs:37,273,297`; `live.rs:354,473,530` uses the fixed set for trials and report effects. `report.rs:9,111-116` fixes the comparison baseline as C; the decision arm is fixed in `trial.rs:33`. These are the same fixed-arm assumption, not separate findings.
- **Reachable consequence:** A consumer invoking the exported `evaluate` or `live::run` can only evaluate S/C/E/F. It cannot supply a named new `Ranker` to compare, so the crate is the moved experiment rather than the required reusable harness.
- **Testable correction:** Make the evaluation entry point take named `&dyn Ranker` arms, run trials and compute corresponding report effects from those names, retaining S/C/E/F as the thin live binary's selected arms and preserving the existing C/E/F report. A focused test passes a custom ranker and verifies its named trial and effect output.

### TASK-B-2 — VIOLATION: evaluation crate owns direct provider REST reads

- **Root cause / defect class:** `Api::get` constructs authenticated HTTP requests in the eval crate instead of routing provider access through pipeline types. `Api` has both a direct transport/token and a pipeline `GitHubClient`, which permits bypassing the provider owner.
- **Violated written rule:** `architecture/bohmian-code-review-design.md:24` says eval “reads the provider only through pipeline types”; Decision 3 requires the reusable eval crate under that authority. `AGENTS.md` also assigns Git provider IO solely to the outer pipeline crate.
- **Owner and siblings:** `crates/bohmian-code-review-eval/src/live.rs:147-153,165-190` owns direct GET. Its reached sinks are repository discovery at `live.rs:377`, all-PR paging at `live.rs:387-392`, and base commit time at `live.rs:244-249`. The binary's `live()` constructs the direct transport for that `Api`. All three are one boundary violation.
- **Reachable consequence:** Every live evaluation calls the direct GET path for repository and PR discovery, and sampled trials call it for base commit times. Provider policy, identity checks, and request handling therefore have a second implementation outside the approved owner.
- **Testable correction:** Route those reads through pipeline-owned provider APIs and keep evaluation responsible for sampling and scoring. A no-network fake must show the live evaluation reaches only pipeline provider methods; retain the existing report regression check.

Overall result: **FAIL**. Both findings are direct acceptance or written-boundary violations. No speculative improvements are proposed.

Verification limit: the reported gate is credential-free and does not reproduce the live C/E/F reports. I found no captured live-run output in the review packet; that empirical claim remains unverified here.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":37,"snippet":"pub const ARMS: [(&str, &dyn Ranker); 4] = [","symbols":["ARMS","Ranker"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":273,"snippet":"        arms: ARMS","symbols":["trial","ARMS"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":297,"snippet":"pub fn evaluate(","symbols":["evaluate"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":473,"snippet":"pub async fn run(api: &Api, repos: &[Repository]) -> Result<Vec<Value>, Box<dyn Error>> {","symbols":["run"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":530,"snippet":"    let effects: BTreeMap<String, Value> = ARMS","symbols":["record","ARMS"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":147,"snippet":"pub struct Api {","symbols":["Api"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":165,"snippet":"    async fn get<T: DeserializeOwned>(&self, resource: &str) -> Result<T, Box<dyn Error>> {","symbols":["Api::get"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":168,"snippet":"            .get(HttpRequest {","symbols":["Api::get","HttpRequest"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":244,"snippet":"        let wire: CommitTimeWire = self","symbols":["Api::base_time"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":377,"snippet":"        let wire: RepositoryWire = self.get(&format!(\"/repos/{}\", repo.name)).await?;","symbols":["Api::select"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":387,"snippet":"            let listed: Vec<ListedPr> = self","symbols":["Api::select"]}]
```
