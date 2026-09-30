//! Co-change experiment: select each caller-supplied repository's PR
//! sample, read its time-eligible co-change history, and run its
//! leave-one-out relevance trials under caller-named rankers through the
//! pipeline's GitHub client, read-only.
//!
//! `GITHUB_TOKEN` (required) is a read-only token. Each [`Repository`] has
//! a fixed cutoff and source-language set.
//!
//! For each repository the command reads every PR oldest-created first
//! once, keeps merged default-branch PRs merged before the cutoff, reads
//! each candidate's file listing once, builds its base commit's snapshot in
//! memory, and keeps the [`SAMPLE_SIZE`] most recently merged qualifying
//! PRs. It reads each sampled base commit once for its committer time, then
//! builds one [`History`] through the newest base, reusing listings already
//! read. For each sampled PR it fetches the head blob of every changed
//! source file with changed lines and ranks against the base snapshot's
//! persisted declarations and reference names, so no base blob is read. It
//! runs up to five trials over its withholdable files against the history
//! merged before its base commit. Only after every repository succeeds does
//! it print one source-free JSON report per repository; any run-level
//! failure exits nonzero with no report. Nothing is written to GitHub and no
//! source leaves memory.

use crate::report::{
    self, ADOPTION_THRESHOLD, Arms, ArmsError, BOOTSTRAP_SEED, Effect, Evaluation, RESAMPLES,
    Summary, VALIDITY_THREATS, Verdict,
};
use crate::sample::{Discovery, OmittedReason, SAMPLE_SIZE, SampleError, Screen, eligible};
use crate::trial::{HIT_RANK, RARITY_RULE, TRIAL_RULE, TrialError};
use bohmian_code_review::{
    DF_CEILING, EXTRACTION_VERSION, Error as SymbolicError, FUSION_CONSTANT, Language,
    RelativePath, SymbolicContext, parse_hunks,
};
use bohmian_code_review_pipeline::testing::{MemorySnapshotStore, MemoryStorage};
use bohmian_code_review_pipeline::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildOutcome, FileOutcome, FileStatus,
    GitHubClient, GitHubError, GitHubHost, History, HistoryError, Identity, InstallationId,
    InstallationToken, InvalidIdentity, Listing, ListingReads, MAX_LISTING_PAGES, MergedPr,
    ObjectSha, RepositoryIdentity, RequestLimits, SNAPSHOT_FORMAT_VERSION, Snapshot, SnapshotScope,
    Snapshots, TenantId, TransportError,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use thiserror::Error;

/// Why a live evaluation run failed. Every variant is run-level: the run
/// stops and returns no report. Provider, history, snapshot, sample, trial,
/// and symbolic failures keep their typed cause, so a caller matches the
/// failure class without downcasting or reading the message.
#[derive(Debug, Error)]
pub enum LiveError {
    /// `GITHUB_TOKEN` is unset or not Unicode.
    #[error("GITHUB_TOKEN is required")]
    MissingToken,
    /// A [`Repository::name`] is not `<owner>/<name>`.
    #[error("repository `{0}` must be <owner>/<name>")]
    InvalidRepository(String),
    /// The token or the evaluation identity is invalid.
    #[error(transparent)]
    Identity(#[from] InvalidIdentity),
    /// The GitHub client's transport could not be built.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The caller-named arms are invalid.
    #[error(transparent)]
    Arms(#[from] ArmsError),
    /// A provider read failed or returned an untrusted response.
    #[error(transparent)]
    GitHub(#[from] GitHubError),
    /// A PR file listing or the co-change history it feeds cannot be
    /// trusted.
    #[error(transparent)]
    History(#[from] HistoryError),
    /// A base snapshot could not be built or read back.
    #[error(transparent)]
    Snapshot(#[from] ColdBuildError),
    /// Sampling failed at run level.
    #[error(transparent)]
    Sample(#[from] SampleError),
    /// A base snapshot's persisted facts do not load into the facade.
    #[error(transparent)]
    Symbolic(#[from] SymbolicError),
    /// A relevance trial could not be run.
    #[error(transparent)]
    Trial(#[from] TrialError),
    /// A report could not be serialised.
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

/// One evaluated repository.
pub struct Repository {
    /// `<owner>/<name>`.
    pub name: &'static str,
    /// Only PRs merged strictly before this time are sampled.
    pub cutoff: &'static str,
    /// Languages whose files are its source.
    pub languages: &'static [Language],
    /// Path suffixes its source is limited to, when the languages' routing
    /// admits more.
    pub suffixes: Option<&'static [&'static str]>,
}

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

/// Provider requests of one repository's run, by who would need them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Requests {
    /// Merged-PR listing pages read, once per repository.
    listing_pages: usize,
    /// History file-listing page requests by production step.
    history: ListingReads,
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

/// The read-only sampling command over the pipeline GitHub client, which
/// owns every provider read.
pub struct Api {
    /// Pipeline client for discovery, listings, commits, blobs, and base
    /// snapshots.
    client: GitHubClient,
}

impl Api {
    /// An [`Api`] over `client`.
    pub fn new(client: GitHubClient) -> Api {
        Api { client }
    }

    /// Build `pr`'s complete base snapshot in memory and read it back.
    ///
    /// # Errors
    ///
    /// Returns [`LiveError::Sample`] with [`SampleError::IncompleteSnapshot`]
    /// when the base tree was not fully indexed, and [`LiveError::Snapshot`]
    /// for provider and storage failures.
    async fn base_snapshot(
        &self,
        identity: &Identity,
        pr: &MergedPr,
    ) -> Result<Snapshot, LiveError> {
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

    /// Qualify `pr` from one file listing, recorded in `listed` as history
    /// with its page requests, then from its base snapshot only when the listing alone could
    /// qualify.
    ///
    /// # Errors
    ///
    /// Returns [`LiveError::GitHub`] for provider reads,
    /// [`LiveError::History`] for an untrusted listing, and
    /// [`Api::base_snapshot`]'s errors.
    async fn qualify(
        &self,
        identity: &Identity,
        eligible: impl Fn(&RelativePath) -> bool,
        pr: &MergedPr,
        listed: &mut BTreeMap<u64, (Option<Vec<RelativePath>>, usize)>,
    ) -> Result<Option<Sampled>, LiveError> {
        let (listing, pages) = Listing::read(
            &self.client,
            &identity.repository,
            pr.number,
            MAX_LISTING_PAGES,
        )
        .await?;
        listed.insert(pr.number, (listing.history_paths(pr.number)?, pages));
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
            .client
            .commit(&identity.repository, &pr.base)
            .await?
            .committed_at;
        Ok(Some(Sampled {
            pr: pr.clone(),
            screen,
            base_time,
            snapshot,
        }))
    }

    /// Add `sampled`'s trials to `evaluation` against the part of `history`
    /// merged before its base commit, fetching each head blob it needs that
    /// `cache` lacks through the hash-verifying client.
    ///
    /// # Errors
    ///
    /// Returns [`LiveError::GitHub`] for a blob that is missing, oversize,
    /// or fails verification, [`LiveError::Symbolic`] for snapshot facts
    /// that do not load, and [`LiveError::Trial`].
    async fn trials(
        &self,
        repository: &RepositoryIdentity,
        sampled: &Sampled,
        history: &History,
        cache: &mut BTreeMap<ObjectSha, Vec<u8>>,
        evaluation: &mut Evaluation<'_>,
    ) -> Result<(), LiveError> {
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
        // ponytail: every distinct head blob of the run stays cached,
        // trading memory for requests across PRs sharing a head blob.
        for (_, sha) in &head {
            if !cache.contains_key(*sha) {
                let bytes = self.client.blob(repository, sha, max_file_bytes).await?;
                cache.insert((*sha).clone(), bytes);
            }
        }
        let heads: BTreeMap<RelativePath, &[u8]> = head
            .iter()
            .filter_map(|(path, sha)| Some(((*path).clone(), cache.get(*sha)?.as_slice())))
            .collect();
        let eligible: Vec<&[RelativePath]> = history
            .for_trial(sampled.pr.number, &sampled.base_time)
            .map(|pr| pr.paths.as_slice())
            .collect();
        evaluation.add(
            sampled.pr.number,
            &sampled.snapshot.clone().into_context()?,
            &sampled.screen.source,
            &heads,
            &eligible,
        )?;
        Ok(())
    }

    /// Select `repo`'s identity, sample, and history: read every PR
    /// oldest-created first once, qualify merged PRs in sample order until
    /// [`SAMPLE_SIZE`] qualify, then build history through the newest
    /// sampled base, reading only listings qualification did not.
    ///
    /// # Errors
    ///
    /// Returns [`LiveError::InvalidRepository`] before any read for a
    /// malformed name, [`LiveError::Sample`] with [`SampleError::TooFew`]
    /// when fewer qualify or any other run-level [`SampleError`],
    /// [`LiveError::History`] for an untrusted history listing, and
    /// provider, snapshot, and storage errors.
    async fn select(&self, repo: &Repository) -> Result<Selected, LiveError> {
        let (owner, name) = repo
            .name
            .split_once('/')
            .ok_or_else(|| LiveError::InvalidRepository(repo.name.to_owned()))?;
        let (repository, described) = self.client.find_repository(owner, name).await?;
        let identity = Identity {
            tenant: TenantId::new("eval")?,
            installation: InstallationId::new(1)?,
            repository,
        };
        let symbolic = SymbolicContext::new();
        let mut requests = Requests::default();
        let mut discovery = Discovery::new(&described.default_branch, repo.cutoff)?;
        for page in 1.. {
            let listed = self
                .client
                .pull_requests(&identity.repository, page)
                .await?;
            requests.listing_pages += 1;
            for pr in listed.prs {
                discovery.offer(pr)?;
            }
            if !listed.has_next {
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
            times
                .clone()
                .min()
                .expect("the sample holds SAMPLE_SIZE PRs"),
            times.max().expect("the sample holds SAMPLE_SIZE PRs"),
        );
        let repository = &identity.repository;
        let history = History::build(&merged, cold_point, newest_base, async |number| {
            if let Some(read) = listed.remove(&number) {
                return Ok(read);
            }
            let (listing, pages) =
                Listing::read(&self.client, repository, number, MAX_LISTING_PAGES).await?;
            Ok::<_, LiveError>((listing.history_paths(number)?, pages))
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
/// Returns [`LiveError::MissingToken`], [`LiveError::Identity`] for an
/// unusable token, and [`LiveError::Transport`] for TLS initialisation
/// failures.
pub fn live() -> Result<Api, LiveError> {
    let token = env::var("GITHUB_TOKEN").map_err(|_| LiveError::MissingToken)?;
    Ok(Api::new(GitHubClient::new(
        GitHubHost::public(),
        InstallationToken::new(&token)?,
        RequestLimits::default(),
    )?))
}

/// Select every repository in `repos`, run its trials under `arms`, and
/// return every report only once all repositories succeed, so a failure
/// leaves none. Each report records the repository-wide adoption decision
/// under `arms`' decision key.
///
/// # Errors
///
/// Returns the first repository's [`LiveError`] from selection or trials.
pub async fn run(api: &Api, repos: &[Repository], arms: Arms<'_>) -> Result<Vec<Value>, LiveError> {
    let mut records = Vec::new();
    for repo in repos {
        let selected = api.select(repo).await?;
        let mut cache = BTreeMap::new();
        let mut evaluation = Evaluation::new(arms);
        for sampled in &selected.sample {
            api.trials(
                &selected.repository,
                sampled,
                &selected.history,
                &mut cache,
                &mut evaluation,
            )
            .await?;
        }
        records.push(record(repo, &selected, arms, &evaluation.finish()));
    }
    let verdicts: Vec<Verdict> = records.iter().map(|(_, verdict)| *verdict).collect();
    let (adopt, adopt_key) = (report::adopt(&verdicts), arms.adopt_key());
    Ok(records
        .into_iter()
        .map(|(mut record, _)| {
            record["decision"][&adopt_key] = json!(adopt);
            record
        })
        .collect())
}

/// `repo`'s source-free JSON report of `selected` and its evaluated
/// `summary`, with its decision verdict and rule named by `arms`' decision
/// and baseline; the caller adds the repository-wide adoption.
fn record(
    repo: &Repository,
    selected: &Selected,
    arms: Arms<'_>,
    summary: &Summary,
) -> (Value, Verdict) {
    let means = &summary.means;
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
    let effects: BTreeMap<&String, Value> = summary
        .effects
        .iter()
        .map(|(name, effect)| (name, effect_record(effect)))
        .collect();
    let verdict = summary.verdict;
    let mut decision = serde_json::Map::new();
    decision.insert("rule".to_owned(), json!(arms.rule()));
    decision.insert("threshold".to_owned(), json!(ADOPTION_THRESHOLD));
    decision.insert(arms.verdict_key(), json!(verdict));
    let record = json!({
        "repository": repo.name,
        "cutoff": repo.cutoff,
        "cold_point": selected.history.cold_point,
        "command": "mise run live:context-relevance",
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
        "trials": summary.trials,
        "effects": effects,
        "decision": decision,
        "validity_threats": BTreeMap::from(VALIDITY_THREATS),
    });
    (record, verdict)
}

/// `effect`'s report entry: its mean, interval, and whether the interval
/// contains 0 and so is inconclusive. Only the interval decides this, so
/// the adoption threshold stays with the decision − baseline verdict.
fn effect_record(effect: &Effect) -> Value {
    json!({
        "mean": effect.mean,
        "ci_95": effect.ci_95,
        "inconclusive": report::verdict(effect) == Verdict::Inconclusive,
    })
}

/// Print every live report of `repos` under `arms`.
///
/// # Errors
///
/// Returns [`live`] and [`run`] failures and [`LiveError::Serialize`].
pub async fn print(repos: &[Repository], arms: Arms<'_>) -> Result<(), LiveError> {
    let records = run(&live()?, repos, arms).await?;
    for record in records {
        println!("{}", serde_json::to_string_pretty(&record)?);
    }
    Ok(())
}

/// Command contract checks: the whole selection, history, trial, and
/// report path over a no-network fake GitHub that counts every provider
/// read, and the crate's provider-IO boundary.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review::{CoChange, DEFAULT_RANKER, Ranker, RaritySymbol, SymbolOverlap};
    use bohmian_code_review_pipeline::testing::RoutedTransport;
    use bohmian_code_review_pipeline::{BoxFuture, HttpRequest, HttpResponse, HttpTransport};
    use sha1::{Digest, Sha1};
    use std::sync::Arc;

    /// The fake repository: Python, cutoff after every fixture merge.
    const FAKE: Repository = Repository {
        name: "o/n",
        cutoff: "2026-01-02T00:00:00Z",
        languages: &[Language::Python],
        suffixes: None,
    };
    /// The live experiment's arms.
    const ARMS: [(&str, &dyn Ranker); 4] = [
        ("S", &SymbolOverlap),
        ("C", &RaritySymbol),
        ("E", &CoChange),
        ("F", &DEFAULT_RANKER),
    ];

    /// The live arms paired against C, deciding F.
    fn arms() -> Arms<'static> {
        Arms::new(&ARMS, "C", "F").expect("fixed arms are valid")
    }

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
    /// listed over two pages. Every read goes through the pipeline client's
    /// one counted transport.
    struct Fake {
        client: Arc<RoutedTransport>,
    }

    impl Fake {
        /// Route the whole fake repository.
        fn new() -> Fake {
            let fake = Fake {
                client: Arc::default(),
            };
            let tree = sha(0x7e5, 0);
            fake.client.json(
                "/repos/o/n",
                r#"{"id":1,"full_name":"o/n","default_branch":"main","private":false}"#,
            );
            let listed: Vec<_> = (1..=65)
                .map(|n| {
                    json!({"number": n, "updated_at": at(n, 30), "merged_at": at(n, 30),
                    "base": {"ref": "main", "sha": sha(0xba5e, n), "repo": {"id": 1}},
                    "head": {"ref": "topic", "sha": sha(0x4ead, n)}})
                })
                .collect();
            fake.client.json(
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
                "/pulls?state=closed&base=main&sort=updated&direction=desc&per_page=100&page=1",
                "[]",
            );
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
                fake.client.json(
                    &format!("/repos/o/n/git/commits/{base}"),
                    &json!({"sha": base, "tree": {"sha": tree}, "committer": {"date": at(n, 0)}})
                        .to_string(),
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
            Api::new(GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                RequestLimits::default(),
                Box::new(Shared(Arc::clone(&self.client))),
            ))
        }

        /// Client URLs requested ending with `suffix`.
        fn reads(&self, suffix: &str) -> usize {
            self.client
                .seen()
                .iter()
                .filter(|url| url.ends_with(suffix))
                .count()
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

    /// One run over the fake discovers the repository and reads the all-PR
    /// listing once, each needed PR file listing once (both pages of the
    /// history-only cold mass edit and of the cached incremental PR 40),
    /// and each sampled base commit once beyond its cold build, all through
    /// the pipeline client, with no provider read per trial; its report
    /// counts every page request in cold, per-PR incremental, and aggregate
    /// incremental costs, apart from review and evaluation-only costs.
    #[tokio::test]
    async fn run_reads_each_input_once_and_reports_costs() {
        let fake = Fake::new();
        let records = run(&fake.api(), &[FAKE], arms())
            .await
            .expect("fake run succeeds");

        assert_eq!(
            fake.client.seen().first().map(String::as_str),
            Some("https://api.github.com/repos/o/n")
        );
        assert_eq!(
            fake.reads("/pulls?state=all&sort=created&direction=asc&per_page=100&page=1"),
            1
        );
        // Each sampled base is pinned once by its cold build and read once
        // more for its evaluation-only committer time.
        for n in 16..=65 {
            let commit = format!("/git/commits/{}", sha(0xba5e, n));
            assert_eq!(fake.reads(&commit), 2, "PR {n}");
        }
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
        let records = run(&fake.api(), &[FAKE], arms())
            .await
            .expect("fake run succeeds");
        let [record] = records.as_slice() else {
            panic!("one report per repository");
        };

        assert_eq!(record["cutoff"], FAKE.cutoff);
        assert_eq!(record["command"], "mise run live:context-relevance");
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
            for arm in ["S", "C", "E", "F"] {
                assert!(pr["hit_at_10_means"][arm].is_f64(), "PR {n} {arm}");
            }
        }
        assert_eq!(sample[SAMPLE_SIZE - 1]["history_size"], 10);
        assert_eq!(sample[0]["history_size"], 10 + 49);
        for trial in record["trials"].as_array().expect("trials") {
            assert!(trial["withheld"].is_string());
            for arm in ["S", "C", "E", "F"] {
                assert!(trial[arm]["hit_at_10"].is_boolean());
            }
        }
        assert_eq!(
            record["effects"]
                .as_object()
                .expect("effects")
                .keys()
                .collect::<Vec<_>>(),
            ["E-C", "F-C", "S-C"]
        );
        for effect in ["E-C", "F-C", "S-C"] {
            let reported = &record["effects"][effect];
            assert!(reported["mean"].is_f64(), "{effect}");
            let [low, high] = [0, 1].map(|end| reported["ci_95"][end].as_f64().expect("bound"));
            assert_eq!(reported["inconclusive"], low <= 0.0 && high >= 0.0);
        }
        let straddles = Effect {
            mean: 0.2,
            ci_95: [-0.1, 0.5],
        };
        let excludes = Effect {
            mean: 0.2,
            ci_95: [0.1, 0.3],
        };
        assert_eq!(
            effect_record(&straddles),
            json!({"mean": 0.2, "ci_95": [-0.1, 0.5], "inconclusive": true})
        );
        assert_eq!(effect_record(&excludes)["inconclusive"], false);
        let (e_record, f_verdict) = (effect_record(&excludes), report::verdict(&straddles));
        assert!(e_record.get("verdict").is_none(), "E carries no F decision");
        assert!(!report::adopt(&[f_verdict]), "E cannot adopt F");
        let decision = record["decision"].as_object().expect("decision");
        assert_eq!(
            decision.keys().collect::<Vec<_>>(),
            ["adopt_f", "repository_f_minus_c", "rule", "threshold"]
        );
        assert_eq!(
            decision["rule"],
            "adopt F only if, in every repository, the mean per-PR F - C hit@10 difference is at least 0.05 and its 95% bootstrap interval's lower bound is above 0; an interval containing 0 is inconclusive"
        );
        assert_eq!(decision["threshold"], ADOPTION_THRESHOLD);
        assert!(decision["repository_f_minus_c"].is_string());
        assert!(decision["adopt_f"].is_boolean());
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

    /// A run under caller-named arms other than C/F names every decision
    /// field after that pair: the effect, the repository verdict, the
    /// aggregate adoption, and the rule.
    #[tokio::test]
    async fn run_report_names_the_callers_decision_pair() {
        let rankers: [(&str, &dyn Ranker); 2] =
            [("Overlap", &SymbolOverlap), ("Fused", &DEFAULT_RANKER)];
        let arms = Arms::new(&rankers, "Overlap", "Fused").expect("valid arms");
        let fake = Fake::new();
        let records = run(&fake.api(), &[FAKE], arms)
            .await
            .expect("fake run succeeds");
        let [record] = records.as_slice() else {
            panic!("one report per repository");
        };

        assert_eq!(
            record["effects"]
                .as_object()
                .expect("effects")
                .keys()
                .collect::<Vec<_>>(),
            ["Fused-Overlap"]
        );
        let decision = record["decision"].as_object().expect("decision");
        assert_eq!(
            decision.keys().collect::<Vec<_>>(),
            [
                "adopt_fused",
                "repository_fused_minus_overlap",
                "rule",
                "threshold"
            ]
        );
        assert_eq!(
            decision["rule"],
            "adopt Fused only if, in every repository, the mean per-PR Fused - Overlap hit@10 difference is at least 0.05 and its 95% bootstrap interval's lower bound is above 0; an interval containing 0 is inconclusive"
        );
        let verdict = decision["repository_fused_minus_overlap"].clone();
        assert!(verdict.is_string());
        assert_eq!(decision["adopt_fused"], verdict == "met");
    }

    /// A missing base-commit response, a history-only (PR 14) or cached
    /// qualification (PR 30) listing still paginated at the page ceiling,
    /// and a rate limit the client cannot wait out each fail the run with
    /// no report.
    #[tokio::test]
    async fn run_fails_without_report_on_missing_incomplete_or_rate_limited_reads() {
        let base = sha(0xba5e, 40);
        let pinned =
            json!({"sha": base, "tree": {"sha": sha(0x7e5, 0)}, "committer": {"date": at(40, 0)}});
        let other = json!({"sha": sha(0xba5e, 41), "tree": {"sha": sha(0x7e5, 0)}, "committer": {"date": at(40, 0)}});
        for (status, body) in [(404, String::new()), (200, other.to_string())] {
            let missing = Fake::new();
            missing.client.respond(
                &format!("/repos/o/n/git/commits/{base}"),
                vec![
                    (200, Vec::new(), pinned.to_string().into_bytes()),
                    (status, Vec::new(), body.into_bytes()),
                ],
            );
            let error = run(&missing.api(), &[FAKE], arms())
                .await
                .expect_err("unusable base time fails");
            assert!(
                matches!(
                    error,
                    LiveError::GitHub(
                        GitHubError::NotFound { .. } | GitHubError::IdentityMismatch { .. }
                    )
                ),
                "{error}"
            );
        }

        for number in [14, 30] {
            let incomplete = Fake::new();
            for page in 1..=MAX_LISTING_PAGES {
                incomplete.client.respond(
                    &files_page(number, page),
                    vec![(
                        200,
                        vec![("link".to_owned(), r#"<https://x/>; rel="next""#.to_owned())],
                        entries(&[format!("docs/{page}.md")]).into_bytes(),
                    )],
                );
            }
            let error = run(&incomplete.api(), &[FAKE], arms())
                .await
                .expect_err("incomplete listing fails");
            assert!(
                matches!(
                    error,
                    LiveError::History(HistoryError::IncompleteListing { number: failed })
                        if failed == number
                ),
                "{error}"
            );
            let pages = incomplete
                .listing_reads()
                .iter()
                .filter(|url| url.contains(&format!("/pulls/{number}/files?")))
                .count();
            assert_eq!(pages, MAX_LISTING_PAGES as usize);
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
        let error = run(&limited.api(), &[FAKE], arms())
            .await
            .expect_err("rate limit fails");
        assert!(
            matches!(error, LiveError::GitHub(GitHubError::RateLimited { .. })),
            "{error}"
        );
    }

    /// Repository discovery and the all-PR listing are verified pipeline
    /// reads: a repository answering as another, or a listing entry merged
    /// after its last update, fails the run with no report and no file
    /// listing read.
    #[tokio::test]
    async fn discovery_reads_are_verified_by_the_pipeline_client() {
        let renamed = Fake::new();
        renamed.client.json(
            "/repos/o/n",
            r#"{"id":1,"full_name":"x/y","default_branch":"main","private":false}"#,
        );
        let error = run(&renamed.api(), &[FAKE], arms())
            .await
            .expect_err("another repository fails");
        assert!(matches!(
            error,
            LiveError::GitHub(GitHubError::IdentityMismatch { .. })
        ));

        let impossible = Fake::new();
        impossible.client.json(
            "/repos/o/n/pulls?state=all&sort=created&direction=asc&per_page=100&page=1",
            &json!([{"number": 1, "updated_at": at(1, 0), "merged_at": at(1, 30),
                "base": {"ref": "main", "sha": sha(0xba5e, 1), "repo": {"id": 1}},
                "head": {"ref": "topic", "sha": sha(0x4ead, 1)}}])
            .to_string(),
        );
        let error = run(&impossible.api(), &[FAKE], arms())
            .await
            .expect_err("merge after update fails");
        assert!(matches!(
            error,
            LiveError::GitHub(GitHubError::MalformedResponse { .. })
        ));
        for fake in [renamed, impossible] {
            assert!(fake.listing_reads().is_empty());
        }
    }

    /// A repository name without an owner is a typed configuration failure
    /// raised before any provider read.
    #[tokio::test]
    async fn malformed_repository_name_fails_before_any_read() {
        let fake = Fake::new();
        let unnamed = Repository {
            name: "pydantic",
            ..FAKE
        };
        let error = run(&fake.api(), &[unnamed], arms())
            .await
            .expect_err("malformed name fails");
        assert!(
            matches!(&error, LiveError::InvalidRepository(name) if name == "pydantic"),
            "{error}"
        );
        assert!(fake.client.seen().is_empty());
    }

    /// Provider IO belongs to the pipeline: no production source of this
    /// crate builds, authenticates, sends, or decodes a raw provider
    /// request, nor erases a library error behind `dyn Error`.
    #[test]
    fn eval_has_no_raw_provider_request_path() {
        let sources = [
            ("lib.rs", include_str!("lib.rs")),
            ("live.rs", include_str!("live.rs")),
            ("report.rs", include_str!("report.rs")),
            ("sample.rs", include_str!("sample.rs")),
            ("trial.rs", include_str!("trial.rs")),
            (
                "bin/context_relevance.rs",
                include_str!("bin/context_relevance.rs"),
            ),
        ];
        for (file, source) in sources {
            let production = source.split("#[cfg(test)]").next().unwrap_or(source);
            for raw in [
                "HttpRequest",
                "HttpTransport",
                "ReqwestTransport",
                "reqwest",
                "api_base",
                "authorization",
                "Bearer",
                "Deserialize",
                "from_slice",
                "/repos/",
                "dyn Error",
                "std::error",
            ] {
                assert!(!production.contains(raw), "{file} uses `{raw}`");
            }
        }
    }
}
