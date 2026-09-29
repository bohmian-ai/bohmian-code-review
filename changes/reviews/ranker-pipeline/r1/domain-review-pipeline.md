# Domain review: pipeline consumer and evaluation boundary

Subject: base `322a2b9e0984062625d54274798a9c32cb01142d` through candidate `100249c3031d645ce8db9148b4b6fa43e71481d1`.

## Reviewed boundary and authority

Reviewed decisions 3 and 8 in `changes/reviews/ranker-pipeline/intent.md`, decision 7 where the wrapper calls update and PR context; `architecture/references/domain/pipeline-consumer-ergonomics.md` and its review checklist; `AGENTS.md:23`; `architecture/bohmian-code-review-design.md:22-24`. Traced `ReviewPipeline::{context,refresh,read}` through `Update`, `PrContext`, and `Contexts`, plus the public pipeline journey. Traced the evaluation crate's `live::run` through sample selection, `trial::evaluate`, `report::{effect,verdict}`, and its provider callers. The complete base-to-candidate diff and changed manifests were considered.

## Result by obligation

| Obligation | Evidence | Result |
|---|---|---|
| 8: one consumer entry point, shared storage/store/limit/error; per-call client | `review.rs:21-174`, facade export `lib.rs:70-71`, `README.md:5` | PASS |
| 8: `context` cold-builds only without pointer; `refresh` updates; `read` uses no client | `review.rs:112-173`, `pipeline_journey.rs:608-717` | PASS |
| 7: refresh extends history and context uses persisted, time-eligible history | `update.rs:68-141`, `pull_request.rs:222-229,430-465`; public journey `pipeline_journey.rs:618-717` | PASS within this domain |
| 3: feature-gated, reusable harness taking a repository sample and named rankers | `eval/Cargo.toml`, `trial.rs:37-42,261-319`, `live.rs:53-63,473-485` | FAIL: DOMAIN-PIPE-1 |
| 3 and architecture: evaluation reads provider only through pipeline types | `live.rs:145-190,238-257,372-400`; `AGENTS.md:23`, design `:22-24` | FAIL: DOMAIN-PIPE-2 |

## Material findings

### DOMAIN-PIPE-1 — evaluation inputs are hardcoded

**Classification:** MISSING. **Defect class:** the evaluation owner's public input shape does not carry the ranker arms or a caller-supplied sample; fixed global experiment state supplies both. **Root cause:** `trial::evaluate` accepts one PR's prepared inputs and `trial` always loops over global `ARMS`. `live::run` accepts repository descriptors, but sample selection happens internally; `Repository` fields are private, so an external caller cannot construct a different descriptor. The live report independently iterates global `ARMS` and fixes C and F for effects and decision.

**Siblings:** `trial.rs:37-42` defines the fixed arms, `:261-279` consumes them, `:297-319` omits an arm parameter; `live.rs:53-63` limits repository descriptors, `:473-485` selects internally, and `:529-539` derives report effects from fixed arms; `report.rs:9,113-117` hardcodes the baseline. These are one input-contract gap, not separate findings.

**Violated obligation:** intent decision 3 requires a reusable hit@10 harness that takes a repository sample and named `&dyn Ranker`s. **Observable consequence:** a library consumer cannot evaluate its own named ranker or supplied sample through the crate's public harness. Editing crate source is necessary to change arms. **Testable correction:** make the reusable evaluation entry point accept a repository sample and named ranker references, propagate those through trial scoring and report construction while keeping the live binary's S/C/E/F choices and existing report shape as its defaults; test a caller-supplied arm/sample through the public entry point.

### DOMAIN-PIPE-2 — evaluation crate owns direct GitHub provider reads

**Classification:** VIOLATION. **Defect class:** provider access and response parsing are implemented outside the pipeline owner. **Root cause:** `live::Api` owns a second `HttpTransport` and token, and `Api::get` builds authenticated GitHub REST requests and decodes responses itself, bypassing `GitHubClient`'s provider boundary. Although `HttpTransport` and `HttpRequest` types come from the pipeline crate, the evaluation crate itself chooses URLs, headers, limits, status handling, and wire decoding.

**Siblings:** all direct-read sinks are `live.rs:377` (`/repos/{owner}/{name}` metadata), `:387-400` (paged `/pulls` discovery), and `:244-249` (base commit time); all route through `Api::get` at `:165-190`. Other evaluation source reads use pipeline stage facades: `ColdBuild::run_at` and `Snapshots::read` at `:199-229`, and `GitHubClient` through history, PR file, and blob paths. They do not share this defect.

**Violated rule:** `AGENTS.md:23` says the pipeline is the only crate permitted Git provider IO and eval reads the provider only through pipeline types; `architecture/bohmian-code-review-design.md:22-24` gives the pipeline sole Git provider IO ownership. **Observable consequence:** the feature-gated evaluation crate contains a separate GitHub client with its own provider policy and request semantics, so pipeline provider handling no longer governs all evaluation reads. **Testable correction:** route the three direct-read operations through the pipeline's GitHub access owner while preserving the live sampling and report behavior; remove the evaluation-local direct transport, token, and REST decoder; verify the no-network fake-provider journey still covers all three operations.

## Verification limits

The parent orchestrator reports that `mise run gate` passed at candidate HEAD and `git diff --check` passed over the range. This review did not rerun the full gate. The live GitHub experiment and exact checked-in report reproduction were not independently rerun here because they require a token; the fake-provider eval lane in the gate covers its local behavior, not live parity. No source files were changed.

**Overall: FAIL.**

```citations
[{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":37,"snippet":"pub const ARMS: [(&str, &dyn Ranker); 4] = [","symbols":["ARMS"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":273,"snippet":"        arms: ARMS","symbols":["trial","ARMS"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":297,"snippet":"pub fn evaluate(","symbols":["evaluate"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":53,"snippet":"pub struct Repository {","symbols":["Repository"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":473,"snippet":"pub async fn run(api: &Api, repos: &[Repository]) -> Result<Vec<Value>, Box<dyn Error>> {","symbols":["run"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":530,"snippet":"    let effects: BTreeMap<String, Value> = ARMS","symbols":["record","ARMS"]},{"id":"DOMAIN-PIPE-1","path":"crates/bohmian-code-review-eval/src/report.rs","line":116,"snippet":"        .map(|pr| pr.arm(treatment) - pr.arm(BASELINE))","symbols":["effect","BASELINE"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":147,"snippet":"pub struct Api {","symbols":["Api"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":165,"snippet":"    async fn get<T: DeserializeOwned>(&self, resource: &str) -> Result<T, Box<dyn Error>> {","symbols":["Api","get"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":168,"snippet":"            .get(HttpRequest {","symbols":["Api","get","HttpRequest"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":244,"snippet":"        let wire: CommitTimeWire = self","symbols":["Api","base_time"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":377,"snippet":"        let wire: RepositoryWire = self.get(&format!(\"/repos/{}\", repo.name)).await?;","symbols":["Api","select"]},{"id":"DOMAIN-PIPE-2","path":"crates/bohmian-code-review-eval/src/live.rs","line":387,"snippet":"            let listed: Vec<ListedPr> = self","symbols":["Api","select"]}]
```
