//! Co-change experiment: select each fixed repository's PR sample, read its
//! time-eligible co-change history, and run its leave-one-out relevance
//! trials through the real GitHub API, read-only.
//!
//! `GITHUB_TOKEN` (required) is a read-only token. Each repository in
//! [`REPOSITORIES`] has a fixed cutoff and source-language set.
//!
//! For each repository the command reads every PR oldest-created first
//! once, keeps merged default-branch PRs merged before the cutoff, reads
//! each candidate's file listing once, builds its base commit's snapshot in
//! memory, and keeps the [`SAMPLE_SIZE`] most recently merged qualifying
//! PRs. It reads each sampled base commit once for its committer time, then
//! builds one [`History`] through the newest base, reusing listings already
//! read. For each sampled PR it fetches the hash-verified base blob of every
//! indexed file and the head blob of every changed source file with changed
//! lines, then runs up to five trials over its withholdable files against the
//! history merged before its base commit. Only after every
//! repository succeeds does it print one source-free JSON report per
//! repository; any run-level failure exits nonzero with no report. Nothing
//! is written to GitHub and no source leaves memory.

mod history;
mod report;
mod sample;
mod trial;

use bohmian_code_review::{
    EXTRACTION_VERSION, Language, RelativePath, SymbolicContext, parse_hunks,
};
use bohmian_code_review_pipeline::testing::{MemorySnapshotStore, MemoryStorage};
use bohmian_code_review_pipeline::{
    ColdBuild, ColdBuildLimits, ColdBuildOutcome, FileOutcome, FileStatus, GitHubClient,
    GitHubHost, HttpRequest, HttpTransport, Identity, InstallationId, InstallationToken, ObjectSha,
    RepositoryIdentity, RequestLimits, ReqwestTransport, SNAPSHOT_FORMAT_VERSION, Snapshot,
    SnapshotScope, Snapshots, TenantId,
};
use history::{Changed, History};
use report::{
    ADOPTION_THRESHOLD, BOOTSTRAP_SEED, DECISION_RULE, RESAMPLES, VALIDITY_THREATS, Verdict,
};
use sample::{
    Discovery, ListedPr, Listing, MergedPr, OmittedReason, SAMPLE_SIZE, SampleError, Screen,
    eligible, is_timestamp,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use trial::{DF_CEILING, FUSION_CONSTANT, HIT_RANK, RARITY_RULE, TRIAL_RULE, Trial, evaluate};

/// One evaluated repository.
struct Repository {
    /// `<owner>/<name>`.
    name: &'static str,
    /// Only PRs merged strictly before this time are sampled.
    cutoff: &'static str,
    /// Languages whose files are its source.
    languages: &'static [Language],
    /// Path suffixes its source is limited to, when the languages' routing
    /// admits more.
    suffixes: Option<&'static [&'static str]>,
}

/// Repositories evaluated, with their fixed cutoffs and source languages.
const REPOSITORIES: [Repository; 3] = [
    Repository {
        name: "pydantic/pydantic-ai",
        cutoff: "2026-08-23T19:15:42Z",
        languages: &[Language::Python],
        suffixes: None,
    },
    Repository {
        name: "tokio-rs/tokio",
        cutoff: "2026-05-21T13:41:55Z",
        languages: &[Language::Rust],
        suffixes: None,
    },
    Repository {
        name: "colinhacks/zod",
        cutoff: "2026-09-01T00:00:00Z",
        languages: &[Language::Typescript, Language::Tsx],
        suffixes: Some(&[".ts", ".tsx"]),
    },
];
/// PRs per discovery page, GitHub's maximum.
const PRS_PER_PAGE: usize = 100;
/// File-listing pages read at most: 3,000 files at 100 per page.
const MAX_FILE_PAGES: u32 = 30;

/// A qualifying PR with the inputs its relevance trials run on.
struct Sampled {
    /// Validated identity and commits.
    pr: MergedPr,
    /// Its source files and patch omissions.
    screen: Screen,
    /// Its base commit's committer time.
    base_time: String,
    /// Complete declaration facts of its base commit.
    snapshot: Snapshot,
}

/// Wire shape of `GET /repos/{owner}/{repo}`.
#[derive(Deserialize)]
struct RepositoryWire {
    id: u64,
    default_branch: String,
}

/// Wire shape of `GET /repos/{owner}/{repo}/git/commits/{sha}`, reduced to
/// the committer time.
#[derive(Deserialize)]
struct CommitTimeWire {
    sha: String,
    committer: CommitterWire,
}

/// Wire shape of a commit's committer.
#[derive(Deserialize)]
struct CommitterWire {
    date: String,
}

/// Provider requests of one repository's run, by who would need them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Requests {
    /// Merged-PR listing pages read, once per repository.
    listing_pages: usize,
    /// History file-listing page requests by production step.
    history: history::ListingReads,
    /// Evaluation-only base-commit reads for committer times.
    timestamp_reads: usize,
}

/// One repository's selected sample and the history its trials read.
struct Selected {
    /// Validated repository identity.
    repository: RepositoryIdentity,
    /// Qualifying PRs in sample order.
    sample: Vec<Sampled>,
    /// Co-change history through the newest sampled base.
    history: History,
    /// Provider requests the selection made.
    requests: Requests,
}

/// The read-only sampling command: the pipeline GitHub client and direct
/// REST reads it does not offer.
struct Api {
    /// Transport for direct REST reads: `reqwest` live, a fake in tests.
    direct: Box<dyn HttpTransport>,
    /// Read-only token for direct REST reads.
    token: String,
    /// Pipeline client for PR file listings and base snapshots.
    client: GitHubClient,
}

impl Api {
    /// GET `resource` from the public API once and decode its JSON body,
    /// read no further than the client's `max_json_bytes` limit.
    ///
    /// # Errors
    ///
    /// Returns transport, non-200 status, oversize body, and decoding
    /// failures. An oversize body fails at the chunk that would pass the
    /// limit, before that chunk is buffered.
    async fn get<T: DeserializeOwned>(&self, resource: &str) -> Result<T, Box<dyn Error>> {
        let mut response = self
            .direct
            .get(HttpRequest {
                url: format!("{}{resource}", GitHubHost::public().api_base()),
                headers: vec![
                    ("authorization", format!("Bearer {}", self.token)),
                    ("accept", "application/vnd.github+json".to_owned()),
                    ("x-github-api-version", "2022-11-28".to_owned()),
                    ("user-agent", "bohmian-code-review-eval".to_owned()),
                ],
                timeout: RequestLimits::default().request_timeout,
            })
            .await?;
        if response.status != 200 {
            return Err(format!("GET {resource} returned status {}", response.status).into());
        }
        let limit = RequestLimits::default().max_json_bytes;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.body.chunk().await? {
            if chunk.len() > limit - bytes.len() {
                return Err(format!("GET {resource} body exceeds {limit} bytes").into());
            }
            bytes.extend(chunk);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Read PR `number`'s file listing once, following at most `max_pages`
    /// pages until [`Listing::push`] ends it; returns it with the page
    /// requests it took.
    ///
    /// # Errors
    ///
    /// Returns provider errors.
    async fn listing(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
        max_pages: u32,
    ) -> Result<(Listing, usize), Box<dyn Error>> {
        let mut listing = Listing::default();
        let mut pages = 0;
        for page in 1..=max_pages {
            let read = self
                .client
                .pull_request_files(repository, number, page)
                .await?;
            pages += 1;
            if !listing.push(read, page == max_pages) {
                break;
            }
        }
        Ok((listing, pages))
    }

    /// Build `pr`'s complete base snapshot in memory and read it back.
    ///
    /// # Errors
    ///
    /// Returns [`SampleError::IncompleteSnapshot`] when the base tree was not
    /// fully indexed, plus provider and storage errors.
    async fn base_snapshot(
        &self,
        identity: &Identity,
        pr: &MergedPr,
    ) -> Result<Snapshot, Box<dyn Error>> {
        let (storage, store) = (MemoryStorage::default(), MemorySnapshotStore::default());
        let limits = ColdBuildLimits::default();
        let max_file_bytes = limits.max_file_bytes;
        let built = ColdBuild::new(&self.client, &storage, &store, limits)
            .run_at(identity, &pr.base)
            .await?;
        let pointer = match built.outcome {
            ColdBuildOutcome::Published(pointer) | ColdBuildOutcome::Reused(pointer) => pointer,
            ColdBuildOutcome::Incomplete(reason) => {
                return Err(SampleError::IncompleteSnapshot {
                    number: pr.number,
                    base: pr.base.clone(),
                    detail: format!("{reason:?}"),
                }
                .into());
            }
        };
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
            max_file_bytes,
        };
        Ok(Snapshots::new(&storage, &store)
            .read(&scope, &pointer)
            .await?)
    }

    /// Read `sha`'s committer time once.
    ///
    /// # Errors
    ///
    /// [`SampleError::UnusablePr`] for PR `number` when the provider returns
    /// another commit or an unusable time, plus provider errors.
    async fn base_time(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
        sha: &ObjectSha,
    ) -> Result<String, Box<dyn Error>> {
        let wire: CommitTimeWire = self
            .get(&format!(
                "/repos/{}/git/commits/{sha}",
                repository.full_name()
            ))
            .await?;
        if wire.sha != sha.as_str() || !is_timestamp(&wire.committer.date) {
            return Err(SampleError::UnusablePr {
                number,
                detail: format!("base commit {} at `{}`", wire.sha, wire.committer.date),
            }
            .into());
        }
        Ok(wire.committer.date)
    }

    /// Qualify `pr` from one file listing, recorded in `listed` as history
    /// with its page requests, then from its base snapshot only when the listing alone could
    /// qualify.
    ///
    /// # Errors
    ///
    /// Returns run-level [`SampleError`]s and provider or storage errors.
    async fn qualify(
        &self,
        identity: &Identity,
        eligible: impl Fn(&RelativePath) -> bool,
        pr: &MergedPr,
        listed: &mut BTreeMap<u64, (Changed, usize)>,
    ) -> Result<Option<Sampled>, Box<dyn Error>> {
        let (listing, pages) = self
            .listing(&identity.repository, pr.number, MAX_FILE_PAGES)
            .await?;
        listed.insert(pr.number, (history::changed(pr.number, &listing)?, pages));
        let screen = Screen::new(pr.number, listing, eligible)?;
        if !screen.plausible() {
            return Ok(None);
        }
        let snapshot = self.base_snapshot(identity, pr).await?;
        let declared: BTreeSet<&RelativePath> = snapshot
            .files
            .iter()
            .filter_map(|outcome| match outcome {
                FileOutcome::Indexed(file) => Some(&file.facts().path),
                FileOutcome::Skipped(_) => None,
            })
            .collect();
        if !screen.qualifies(|path| declared.contains(path)) {
            return Ok(None);
        }
        let base_time = self
            .base_time(&identity.repository, pr.number, &pr.base)
            .await?;
        Ok(Some(Sampled {
            pr: pr.clone(),
            screen,
            base_time,
            snapshot,
        }))
    }

    /// Run `sampled`'s trials against the part of `history` merged before
    /// its base commit, fetching each base and head blob it needs that
    /// `cache` lacks through the hash-verifying client.
    ///
    /// # Errors
    ///
    /// Returns provider errors, including a blob that is missing, oversize,
    /// or fails verification, and [`trial::TrialError`]s.
    async fn trials(
        &self,
        repository: &RepositoryIdentity,
        sampled: &Sampled,
        history: &History,
        cache: &mut BTreeMap<ObjectSha, Vec<u8>>,
    ) -> Result<Vec<Trial>, Box<dyn Error>> {
        let base: Vec<(&RelativePath, &ObjectSha)> = sampled
            .snapshot
            .files
            .iter()
            .filter_map(|outcome| match outcome {
                FileOutcome::Indexed(file) => Some((&file.facts().path, file.blob())),
                FileOutcome::Skipped(_) => None,
            })
            .collect();
        let head: Vec<(&RelativePath, &ObjectSha)> = sampled
            .screen
            .source
            .iter()
            .filter(|file| {
                file.status != FileStatus::Removed
                    && file.patch.as_deref().is_some_and(|patch| {
                        parse_hunks(patch).is_ok_and(|ranges| !ranges.is_empty())
                    })
            })
            .map(|file| (&file.path, &file.sha))
            .collect();
        let max_file_bytes = ColdBuildLimits::default().max_file_bytes;
        // ponytail: every distinct blob of the run stays cached, trading
        // memory for requests across overlapping base trees.
        for (_, sha) in base.iter().chain(&head) {
            if !cache.contains_key(*sha) {
                let bytes = self.client.blob(repository, sha, max_file_bytes).await?;
                cache.insert((*sha).clone(), bytes);
            }
        }
        let view = |files: &[(&RelativePath, &ObjectSha)]| -> BTreeMap<RelativePath, &[u8]> {
            files
                .iter()
                .filter_map(|(path, sha)| Some(((*path).clone(), cache.get(*sha)?.as_slice())))
                .collect()
        };
        let eligible: Vec<&[RelativePath]> = history
            .for_trial(sampled.pr.number, &sampled.base_time)
            .map(|pr| pr.paths.as_slice())
            .collect();
        Ok(evaluate(
            sampled.pr.number,
            sampled.snapshot.clone().into_context()?,
            &view(&base),
            &sampled.screen.source,
            &view(&head),
            &eligible,
        )?)
    }

    /// Select `repo`'s identity, sample, and history: read every PR
    /// oldest-created first once, qualify merged PRs in sample order until
    /// [`SAMPLE_SIZE`] qualify, then build history through the newest
    /// sampled base, reading only listings qualification did not.
    ///
    /// # Errors
    ///
    /// Returns [`SampleError::TooFew`] when fewer qualify, any other
    /// run-level [`SampleError`], and provider or storage errors.
    async fn select(&self, repo: &Repository) -> Result<Selected, Box<dyn Error>> {
        let (owner, name) = repo
            .name
            .split_once('/')
            .ok_or("repository must be <owner>/<name>")?;
        let wire: RepositoryWire = self.get(&format!("/repos/{}", repo.name)).await?;
        let identity = Identity {
            tenant: TenantId::new("eval")?,
            installation: InstallationId::new(1)?,
            repository: RepositoryIdentity::new(wire.id, owner, name)?,
        };
        let symbolic = SymbolicContext::new();
        let mut requests = Requests::default();
        let mut discovery = Discovery::new(&wire.default_branch, repo.cutoff)?;
        for page in 1.. {
            let listed: Vec<ListedPr> = self
                .get(&format!(
                    "/repos/{}/pulls?state=all&sort=created&direction=asc&per_page={PRS_PER_PAGE}&page={page}",
                    repo.name
                ))
                .await?;
            requests.listing_pages += 1;
            let exhausted = listed.len() < PRS_PER_PAGE;
            for pr in listed {
                discovery.offer(pr)?;
            }
            if exhausted {
                break;
            }
        }
        let merged = discovery.into_sample();
        let mut listed = BTreeMap::new();
        let mut sample = Vec::new();
        for pr in &merged {
            let eligible =
                |path: &RelativePath| eligible(&symbolic, repo.languages, repo.suffixes, path);
            if let Some(sampled) = self.qualify(&identity, eligible, pr, &mut listed).await? {
                sample.push(sampled);
                if sample.len() == SAMPLE_SIZE {
                    break;
                }
            }
        }
        if sample.len() < SAMPLE_SIZE {
            return Err(SampleError::TooFew {
                found: sample.len(),
            }
            .into());
        }
        requests.timestamp_reads = sample.len();
        let times = sample.iter().map(|sampled| sampled.base_time.as_str());
        let (cold_point, newest_base) = (
            times.clone().min().ok_or("empty sample")?,
            times.max().ok_or("empty sample")?,
        );
        let repository = &identity.repository;
        let history = History::build(&merged, cold_point, newest_base, async |number| {
            if let Some(read) = listed.remove(&number) {
                return Ok(read);
            }
            let (listing, pages) = self.listing(repository, number, MAX_FILE_PAGES).await?;
            Ok::<_, Box<dyn Error>>((history::changed(number, &listing)?, pages))
        })
        .await?;
        requests.history = history.reads.clone();
        Ok(Selected {
            repository: identity.repository,
            sample,
            history,
            requests,
        })
    }
}

/// The live command's [`Api`] from `GITHUB_TOKEN`.
///
/// # Errors
///
/// Returns a missing or unusable token and TLS initialisation failures.
fn live() -> Result<Api, Box<dyn Error>> {
    let token = env::var("GITHUB_TOKEN").map_err(|_| "GITHUB_TOKEN is required")?;
    let client = GitHubClient::new(
        GitHubHost::public(),
        InstallationToken::new(&token)?,
        RequestLimits::default(),
    )?;
    Ok(Api {
        direct: Box::new(ReqwestTransport::new()?),
        token,
        client,
    })
}

/// Select every repository in `repos`, run its trials, and return every
/// report only once all repositories succeed, so a failure leaves none.
/// Each report records the repository-wide adoption decision.
///
/// # Errors
///
/// Returns sampling, history, provider, and trial failures.
async fn run(api: &Api, repos: &[Repository]) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut records = Vec::new();
    for repo in repos {
        let selected = api.select(repo).await?;
        let mut cache = BTreeMap::new();
        let mut trials = Vec::new();
        for sampled in &selected.sample {
            trials.extend(
                api.trials(&selected.repository, sampled, &selected.history, &mut cache)
                    .await?,
            );
        }
        records.push(record(repo, &selected, &trials));
    }
    let verdicts: Vec<Verdict> = records.iter().map(|(_, verdict)| *verdict).collect();
    let adopt = report::adopt(&verdicts);
    Ok(records
        .into_iter()
        .map(|(mut record, _)| {
            record["decision"]["adopt_f"] = json!(adopt);
            record
        })
        .collect())
}

/// `repo`'s source-free JSON report of `selected` and its `trials`, with
/// its F − C verdict; the caller adds the repository-wide adoption.
fn record(repo: &Repository, selected: &Selected, trials: &[Trial]) -> (Value, Verdict) {
    let means = report::pr_means(trials);
    let prs: Vec<_> = selected
        .sample
        .iter()
        .map(|sampled| {
            json!({
                "number": sampled.pr.number,
                "merged_at": sampled.pr.merged_at,
                "base": sampled.pr.base.as_str(),
                "base_time": sampled.base_time,
                "head": sampled.pr.head.as_str(),
                "base_tree": sampled.snapshot.tree.as_str(),
                "hit_at_10_means": means.iter().find(|means| means.pr == sampled.pr.number),
                "history_size": selected.history
                    .for_trial(sampled.pr.number, &sampled.base_time)
                    .count(),
                "source_files": sampled.screen.source.iter()
                    .map(|file| file.path.as_str()).collect::<Vec<_>>(),
                "omitted": sampled.screen.omitted.iter().map(|omitted| json!({
                    "path": omitted.path.as_str(),
                    "reason": match &omitted.reason {
                        OmittedReason::MissingPatch => "missing patch".to_owned(),
                        OmittedReason::MalformedPatch(detail) => format!("malformed patch: {detail}"),
                    },
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let requests = &selected.requests;
    let e_minus_c = report::effect(&means, |pr| pr.e, BOOTSTRAP_SEED);
    let f_minus_c = report::effect(&means, |pr| pr.f, BOOTSTRAP_SEED);
    let verdict = report::verdict(&f_minus_c);
    let record = json!({
        "repository": repo.name,
        "cutoff": repo.cutoff,
        "cold_point": selected.history.cold_point,
        "command": "GITHUB_TOKEN=<read-only token> mise run live:context-relevance",
        "outcome": "hit@10",
        "bootstrap": {
            "seed": BOOTSTRAP_SEED,
            "resamples": RESAMPLES,
            "interval": "95% percentile over PRs",
        },
        "requests": {
            "production": {
                "cold_build": requests.listing_pages + requests.history.cold,
                "incremental_update": {
                    "page_requests_by_merged_pr": requests.history.incremental,
                    "aggregate_page_requests_through_newest_base":
                        requests.history.incremental.values().sum::<usize>(),
                },
                "review": 0,
            },
            "evaluation_only": {
                "base_commit_timestamps": requests.timestamp_reads,
            },
        },
        "df_ceiling": DF_CEILING,
        "rarity_rule": RARITY_RULE,
        "trial_rule": TRIAL_RULE,
        "fusion_constant": FUSION_CONSTANT,
        "hit_rank": HIT_RANK,
        "sample": prs,
        "trials": trials,
        "effects": {"E-C": e_minus_c, "F-C": f_minus_c},
        "decision": {
            "rule": DECISION_RULE,
            "threshold": ADOPTION_THRESHOLD,
            "repository_f_minus_c": verdict,
        },
        "validity_threats": BTreeMap::from(VALIDITY_THREATS),
    });
    (record, verdict)
}

/// Print every live report, or only the failure on stderr with exit 1.
///
/// # Errors
///
/// Returns [`live`], [`run`], and serialisation failures.
async fn print() -> Result<(), Box<dyn Error>> {
    let records = run(&live()?, &REPOSITORIES).await?;
    for record in records {
        println!("{}", serde_json::to_string_pretty(&record)?);
    }
    Ok(())
}

/// Entry point: prints the reports, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = print().await {
        eprintln!("context_relevance failed: {error}");
        std::process::exit(1);
    }
}

/// Command contract checks: the fixed repositories, and the whole
/// selection, history, trial, and report path over a no-network fake
/// GitHub that counts every provider read.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review_pipeline::testing::RoutedTransport;
    use bohmian_code_review_pipeline::{BoxFuture, GitHubError, HttpResponse, TransportError};
    use sha1::{Digest, Sha1};
    use std::sync::Arc;

    /// The fake repository: Python, cutoff after every fixture merge.
    const FAKE: Repository = Repository {
        name: "o/n",
        cutoff: "2026-01-02T00:00:00Z",
        languages: &[Language::Python],
        suffixes: None,
    };
    /// Base `src/a.py`.
    const A: &[u8] = b"def alpha():\n    return 1\n";
    /// Base `src/b.py`, calling `alpha`.
    const B: &[u8] = b"from a import alpha\n\ndef beta():\n    return alpha()\n";
    /// Head `src/a.py`: line 2 changed.
    const A_HEAD: &[u8] = b"def alpha():\n    return 2\n";
    /// Head `src/b.py`: line 4 changed.
    const B_HEAD: &[u8] = b"from a import alpha\n\ndef beta():\n    return alpha() + 1\n";

    /// Fake GitHub state shared between a test and the [`Api`] it drives.
    struct Shared(Arc<RoutedTransport>);

    impl HttpTransport for Shared {
        /// Delegates to the shared routed transport.
        ///
        /// # Errors
        ///
        /// Never.
        fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
            self.0.get(request)
        }
    }

    /// Git blob id of `bytes`.
    fn blob_sha(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
        hasher.update(bytes);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// Object id `n` of `kind`.
    fn sha(kind: u32, n: u64) -> String {
        format!("{kind:08x}{n:032x}")
    }

    /// Minute `n` after 2026-01-01T00:00Z at `second`: PR `n` merges at
    /// second 30 and its base commit is at second 0.
    fn at(n: u64, second: u32) -> String {
        format!("2026-01-01T{:02}:{:02}:{second:02}Z", n / 60, n % 60)
    }

    /// PR file-listing page `page` of PR `number`.
    fn files_page(number: u64, page: u32) -> String {
        format!("/repos/o/n/pulls/{number}/files?per_page=100&page={page}")
    }

    /// Listing entries changing `paths`, each with a one-line patch.
    fn entries(paths: &[String]) -> String {
        let entries: Vec<_> = paths
            .iter()
            .map(|path| json!({"filename": path, "status": "modified", "sha": sha(0xd0c, 0), "patch": "@@ -1 +1 @@\n-a\n+b"}))
            .collect();
        serde_json::Value::from(entries).to_string()
    }

    /// A fake repository `o/n` with 65 merged PRs, one per minute. PRs
    /// 16..=65 qualify, modifying `src/a.py` and `src/b.py` on base
    /// commits all holding one tree, and PR 40 lists them over two pages;
    /// PRs 1..=15 change docs only, and PR 15 is a 31-file mass edit
    /// listed over two pages. Direct REST reads
    /// and client reads have separate transports so each can be counted.
    struct Fake {
        direct: Arc<RoutedTransport>,
        client: Arc<RoutedTransport>,
    }

    impl Fake {
        /// Route the whole fake repository.
        fn new() -> Fake {
            let fake = Fake {
                direct: Arc::default(),
                client: Arc::default(),
            };
            let tree = sha(0x7e5, 0);
            fake.direct
                .json("/repos/o/n", r#"{"id":1,"default_branch":"main"}"#);
            fake.client.json(
                "/repos/o/n",
                r#"{"id":1,"full_name":"o/n","default_branch":"main","private":false}"#,
            );
            let listed: Vec<_> = (1..=65)
                .map(|n| {
                    json!({"number": n, "updated_at": at(n, 30), "merged_at": at(n, 30),
                    "base": {"ref": "main", "sha": sha(0xba5e, n)},
                    "head": {"ref": "topic", "sha": sha(0x4ead, n)}})
                })
                .collect();
            fake.direct.json(
                "/repos/o/n/pulls?state=all&sort=created&direction=asc&per_page=100&page=1",
                &serde_json::Value::from(listed).to_string(),
            );
            let tree_entries: Vec<_> = [("src/a.py", A), ("src/b.py", B)]
                .iter()
                .map(|(path, bytes)| {
                    json!({"path": path, "mode": "100644", "type": "blob",
                    "sha": blob_sha(bytes), "size": bytes.len()})
                })
                .collect();
            fake.client.json(
                &format!("/git/trees/{tree}?recursive=1"),
                &json!({"sha": tree, "truncated": false, "tree": tree_entries}).to_string(),
            );
            for bytes in [A, B, A_HEAD, B_HEAD] {
                fake.client
                    .route(&format!("/git/blobs/{}", blob_sha(bytes)), 200, bytes);
            }
            let (a, b) = (
                json!({"filename": "src/a.py", "status": "modified", "sha": blob_sha(A_HEAD),
                 "patch": "@@ -2 +2 @@\n-    return 1\n+    return 2"}),
                json!({"filename": "src/b.py", "status": "modified", "sha": blob_sha(B_HEAD),
                 "patch": "@@ -4 +4 @@\n-    return alpha()\n+    return alpha() + 1"}),
            );
            let changed = json!([a, b]).to_string();
            for n in 16..=65 {
                let base = sha(0xba5e, n);
                fake.direct.json(
                    &format!("/repos/o/n/git/commits/{base}"),
                    &json!({"sha": base, "committer": {"date": at(n, 0)}}).to_string(),
                );
                fake.client.json(
                    &format!("/repos/o/n/git/commits/{base}"),
                    &json!({"sha": base, "tree": {"sha": tree}}).to_string(),
                );
                fake.client.json(&files_page(n, 1), &changed);
            }
            fake.client.respond(
                &files_page(40, 1),
                vec![(
                    200,
                    vec![(
                        "link".to_owned(),
                        r#"<https://x/?page=2>; rel="next""#.to_owned(),
                    )],
                    json!([a]).to_string().into_bytes(),
                )],
            );
            fake.client
                .json(&files_page(40, 2), &json!([b]).to_string());

            for n in 1..=14 {
                fake.client
                    .json(&files_page(n, 1), &entries(&[format!("docs/{n}.md")]));
            }
            let mass: Vec<_> = (0..31).map(|i| format!("docs/mass{i}.md")).collect();
            fake.client.respond(
                &files_page(15, 1),
                vec![(
                    200,
                    vec![(
                        "link".to_owned(),
                        r#"<https://x/?page=2>; rel="next""#.to_owned(),
                    )],
                    entries(&mass[..20]).into_bytes(),
                )],
            );
            fake.client.json(&files_page(15, 2), &entries(&mass[20..]));
            fake
        }

        /// An [`Api`] over this fake.
        fn api(&self) -> Api {
            Api {
                direct: Box::new(Shared(Arc::clone(&self.direct))),
                token: "ghs_token".to_owned(),
                client: GitHubClient::with_transport(
                    GitHubHost::public(),
                    InstallationToken::new("ghs_token").expect("token"),
                    RequestLimits::default(),
                    Box::new(Shared(Arc::clone(&self.client))),
                ),
            }
        }

        /// Client URLs requested for PR file listings, in order.
        fn listing_reads(&self) -> Vec<String> {
            self.client
                .seen()
                .into_iter()
                .filter(|url| url.contains("/files?"))
                .collect()
        }
    }

    /// One run over the fake reads the merged-PR listing once, each
    /// needed PR file listing once (both pages of the history-only cold
    /// mass edit and of the cached incremental PR 40), and each sampled
    /// base commit's time once, with no provider read per trial; its report
    /// counts every page request in cold, per-PR incremental, and aggregate
    /// incremental costs, apart from review and evaluation-only costs.
    #[tokio::test]
    async fn run_reads_each_input_once_and_reports_costs() {
        let fake = Fake::new();
        let records = run(&fake.api(), &[FAKE]).await.expect("fake run succeeds");

        let mut direct = vec![
            "https://api.github.com/repos/o/n".to_owned(),
            "https://api.github.com/repos/o/n/pulls?state=all&sort=created&direction=asc&per_page=100&page=1".to_owned(),
        ];
        direct.extend((16..=65).rev().map(|n| {
            format!(
                "https://api.github.com/repos/o/n/git/commits/{}",
                sha(0xba5e, n)
            )
        }));
        assert_eq!(fake.direct.seen(), direct);
        let page = |n, p| format!("https://api.github.com{}", files_page(n, p));
        let mut listings: Vec<_> = (16..=65)
            .rev()
            .flat_map(|n| {
                if n == 40 {
                    vec![page(n, 1), page(n, 2)]
                } else {
                    vec![page(n, 1)]
                }
            })
            .collect();
        listings.extend([page(15, 1), page(15, 2)]);
        listings.extend((5..=14).rev().map(|n| page(n, 1)));
        assert_eq!(fake.listing_reads(), listings);

        let [record] = records.as_slice() else {
            panic!("one report per repository");
        };
        assert_eq!(record["cold_point"], at(16, 0));
        assert_eq!(record["sample"].as_array().map(Vec::len), Some(SAMPLE_SIZE));
        let incremental: BTreeMap<u64, usize> = (16..=64)
            .map(|n| (n, if n == 40 { 2 } else { 1 }))
            .collect();
        assert_eq!(
            record["requests"],
            json!({
                "production": {
                    "cold_build": 13,
                    "incremental_update": {
                        "page_requests_by_merged_pr": incremental,
                        "aggregate_page_requests_through_newest_base": 50,
                    },
                    "review": 0,
                },
                "evaluation_only": {"base_commit_timestamps": SAMPLE_SIZE},
            })
        );
        let trials = record["trials"].as_array().expect("trials");
        assert_eq!(trials.len(), 2 * SAMPLE_SIZE);
    }

    /// A fake run's report records the rerun inputs, per-PR history sizes
    /// and arm means, both seeded effects, the decision, and every validity
    /// threat, and carries no source text or packing outcome.
    #[tokio::test]
    async fn run_report_is_complete_and_source_free() {
        let fake = Fake::new();
        let records = run(&fake.api(), &[FAKE]).await.expect("fake run succeeds");
        let [record] = records.as_slice() else {
            panic!("one report per repository");
        };

        assert_eq!(record["cutoff"], FAKE.cutoff);
        assert_eq!(
            record["command"],
            "GITHUB_TOKEN=<read-only token> mise run live:context-relevance"
        );
        assert_eq!(record["outcome"], "hit@10");
        assert_eq!(record["bootstrap"]["seed"], BOOTSTRAP_SEED);
        assert_eq!(record["bootstrap"]["resamples"], RESAMPLES);
        let sample = record["sample"].as_array().expect("sample");
        for (pr, n) in sample.iter().zip((16..=65).rev()) {
            assert_eq!(pr["number"], n);
            assert_eq!(pr["base"], sha(0xba5e, n));
            assert_eq!(pr["head"], sha(0x4ead, n));
            assert_eq!(pr["hit_at_10_means"]["pr"], n);
            assert_eq!(pr["hit_at_10_means"]["trials"], 2);
            for arm in ["C", "E", "F"] {
                assert!(pr["hit_at_10_means"][arm].is_f64(), "PR {n} {arm}");
            }
        }
        assert_eq!(sample[SAMPLE_SIZE - 1]["history_size"], 10);
        assert_eq!(sample[0]["history_size"], 10 + 49);
        for trial in record["trials"].as_array().expect("trials") {
            assert!(trial["withheld"].is_string());
            for arm in ["C", "E", "F"] {
                assert!(trial[arm]["hit_at_10"].is_boolean());
            }
        }
        for effect in ["E-C", "F-C"] {
            assert!(record["effects"][effect]["mean"].is_f64(), "{effect}");
            assert_eq!(
                record["effects"][effect]["ci_95"].as_array().map(Vec::len),
                Some(2)
            );
        }
        assert_eq!(record["decision"]["rule"], DECISION_RULE);
        assert_eq!(record["decision"]["threshold"], ADOPTION_THRESHOLD);
        assert!(record["decision"]["repository_f_minus_c"].is_string());
        assert!(record["decision"]["adopt_f"].is_boolean());
        let threats: Vec<&str> = record["validity_threats"]
            .as_object()
            .expect("threats")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            threats,
            [
                "circular_label",
                "one_repository_per_language",
                "proxy_for_relevance",
                "small_sample"
            ]
        );
        let text = record.to_string();
        for source in [A, B, A_HEAD, B_HEAD] {
            let source = std::str::from_utf8(source).expect("fixture is UTF-8");
            for line in source.lines().filter(|line| !line.is_empty()) {
                assert!(!text.contains(line.trim()), "report leaks `{line}`");
            }
        }
        assert!(!text.contains("pack"), "report claims a packing outcome");
        assert!(record.get("summary").is_none());
    }

    /// A missing base-commit response, a history-only (PR 14) or cached
    /// qualification (PR 30) listing still paginated at the page ceiling,
    /// and a rate limit the client cannot wait out each fail the run with
    /// no report.
    #[tokio::test]
    async fn run_fails_without_report_on_missing_incomplete_or_rate_limited_reads() {
        let missing = Fake::new();
        missing.direct.route(
            &format!("/repos/o/n/git/commits/{}", sha(0xba5e, 40)),
            404,
            "",
        );
        let error = run(&missing.api(), &[FAKE])
            .await
            .expect_err("missing read fails");
        assert!(error.to_string().contains("returned status 404"), "{error}");

        for number in [14, 30] {
            let incomplete = Fake::new();
            for page in 1..=MAX_FILE_PAGES {
                incomplete.client.respond(
                    &files_page(number, page),
                    vec![(
                        200,
                        vec![("link".to_owned(), r#"<https://x/>; rel="next""#.to_owned())],
                        entries(&[format!("docs/{page}.md")]).into_bytes(),
                    )],
                );
            }
            let error = run(&incomplete.api(), &[FAKE])
                .await
                .expect_err("incomplete listing fails");
            assert_eq!(
                error.downcast_ref::<SampleError>(),
                Some(&SampleError::IncompleteListing { number })
            );
            let pages = incomplete
                .listing_reads()
                .iter()
                .filter(|url| url.contains(&format!("/pulls/{number}/files?")))
                .count();
            assert_eq!(pages, MAX_FILE_PAGES as usize);
        }

        let limited = Fake::new();
        limited.client.respond(
            &files_page(30, 1),
            vec![(
                429,
                vec![("retry-after".to_owned(), "3600".to_owned())],
                Vec::new(),
            )],
        );
        let error = run(&limited.api(), &[FAKE])
            .await
            .expect_err("rate limit fails");
        assert!(
            matches!(
                error.downcast_ref::<GitHubError>(),
                Some(GitHubError::RateLimited { .. })
            ),
            "{error}"
        );
    }

    /// A direct JSON body of exactly the client's `max_json_bytes` still
    /// decodes; one byte more fails the run with no report. Repository
    /// metadata, merged-PR pages, and commit times all read through
    /// [`Api::get`], so this bounds each of them.
    #[tokio::test]
    async fn direct_json_is_bounded_by_the_client_limit() {
        let limit = RequestLimits::default().max_json_bytes;
        let padded = |len: usize| {
            let mut body = r#"{"id":1,"default_branch":"main"}"#.to_owned();
            body.push_str(&" ".repeat(len - body.len()));
            body
        };

        let at_limit = Fake::new();
        at_limit.direct.json("/repos/o/n", &padded(limit));
        let wire: RepositoryWire = at_limit
            .api()
            .get("/repos/o/n")
            .await
            .expect("body at the limit decodes");
        assert_eq!((wire.id, wire.default_branch.as_str()), (1, "main"));

        let over = Fake::new();
        over.direct.json("/repos/o/n", &padded(limit + 1));
        let error = run(&over.api(), &[FAKE])
            .await
            .expect_err("oversize body fails");
        assert_eq!(
            error.to_string(),
            format!("GET /repos/o/n body exceeds {limit} bytes")
        );
        assert!(over.listing_reads().is_empty());
    }

    /// Each fixed repository has a valid cutoff and a nonempty language
    /// set, and the three samples are the approved ones.
    #[test]
    fn repositories_are_the_fixed_samples() {
        let fixed: Vec<_> = REPOSITORIES
            .iter()
            .map(|repo| {
                assert!(Discovery::new("main", repo.cutoff).is_ok(), "{}", repo.name);
                assert!(!repo.languages.is_empty(), "{}", repo.name);
                (repo.name, repo.cutoff)
            })
            .collect();
        assert_eq!(
            fixed,
            [
                ("pydantic/pydantic-ai", "2026-08-23T19:15:42Z"),
                ("tokio-rs/tokio", "2026-05-21T13:41:55Z"),
                ("colinhacks/zod", "2026-09-01T00:00:00Z"),
            ]
        );
    }
}
