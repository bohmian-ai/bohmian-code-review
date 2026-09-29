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
//! lines, then runs one trial per withholdable file. Only after every
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
    GitHubHost, Identity, InstallationId, InstallationToken, ObjectSha, PrContextLimits,
    RepositoryIdentity, RequestLimits, SNAPSHOT_FORMAT_VERSION, Snapshot, SnapshotScope, Snapshots,
    TenantId,
};
use history::{Changed, History};
use sample::{
    Discovery, ListedPr, Listing, MergedPr, OmittedReason, SAMPLE_SIZE, SampleError, Screen,
    eligible, is_timestamp,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use trial::{
    CANDIDATE_ALLOWANCE, DF_CEILING, PROMPT_RESERVE, RARITY_RULE, TOKEN_ESTIMATE, Trial,
    estimate_tokens, evaluate,
};

/// One evaluated repository.
struct Repository {
    /// `<owner>/<name>`.
    name: &'static str,
    /// Only PRs merged strictly before this time are sampled.
    cutoff: &'static str,
    /// Languages whose files are its source.
    languages: &'static [Language],
}

/// Repositories evaluated, with their fixed cutoffs and source languages.
const REPOSITORIES: [Repository; 3] = [
    Repository {
        name: "pydantic/pydantic-ai",
        cutoff: "2026-08-23T19:15:42Z",
        languages: &[Language::Python],
    },
    Repository {
        name: "tokio-rs/tokio",
        cutoff: "2026-05-21T13:41:55Z",
        languages: &[Language::Rust],
    },
    Repository {
        name: "colinhacks/zod",
        cutoff: "2026-09-01T00:00:00Z",
        languages: &[Language::Typescript, Language::Tsx],
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
    /// Estimated tokens of every patch in its listing.
    patch_tokens: usize,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Requests {
    /// Merged-PR listing pages read, once per repository.
    listing_pages: usize,
    /// History file-listing reads by production step.
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
    /// Transport for direct REST reads.
    http: reqwest::Client,
    /// Read-only token for direct REST reads.
    token: String,
    /// Pipeline client for PR file listings and base snapshots.
    client: GitHubClient,
}

impl Api {
    /// GET `resource` from the public API and decode its JSON body.
    ///
    /// # Errors
    ///
    /// Returns transport, non-success status, and decoding failures.
    async fn get<T: DeserializeOwned>(&self, resource: &str) -> Result<T, Box<dyn Error>> {
        let bytes = self
            .http
            .get(format!("https://api.github.com{resource}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "bohmian-code-review-eval")
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Read PR `number`'s file listing once, following at most `max_pages`
    /// pages until [`Listing::push`] ends it.
    ///
    /// # Errors
    ///
    /// Returns provider errors.
    async fn listing(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
        max_pages: u32,
    ) -> Result<Listing, Box<dyn Error>> {
        let mut listing = Listing::default();
        for page in 1..=max_pages {
            let read = self
                .client
                .pull_request_files(repository, number, page)
                .await?;
            if !listing.push(read, page == max_pages) {
                break;
            }
        }
        Ok(listing)
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

    /// Qualify `pr` from one file listing, recorded in `listed` as history,
    /// then from its base snapshot only when the listing alone could
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
        listed: &mut BTreeMap<u64, Changed>,
    ) -> Result<Option<Sampled>, Box<dyn Error>> {
        let listing = self
            .listing(&identity.repository, pr.number, MAX_FILE_PAGES)
            .await?;
        listed.insert(pr.number, history::changed(pr.number, &listing)?);
        let patch_tokens = listing
            .files
            .iter()
            .filter_map(|file| file.patch.as_deref())
            .map(estimate_tokens)
            .sum();
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
            patch_tokens,
        }))
    }

    /// Run `sampled`'s trials, fetching each base and head blob it needs
    /// that `cache` lacks through the hash-verifying client.
    ///
    /// # Errors
    ///
    /// Returns provider errors, including a blob that is missing, oversize,
    /// or fails verification, and [`trial::TrialError`]s.
    async fn trials(
        &self,
        repository: &RepositoryIdentity,
        sampled: &Sampled,
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
        Ok(evaluate(
            sampled.pr.number,
            sampled.snapshot.clone().into_context()?,
            &view(&base),
            &sampled.screen.source,
            &view(&head),
            sampled.patch_tokens,
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
            let eligible = |path: &RelativePath| eligible(&symbolic, repo.languages, path);
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
            if let Some(changed) = listed.remove(&number) {
                return Ok(changed);
            }
            let listing = self.listing(repository, number, 1).await?;
            Ok::<_, Box<dyn Error>>(history::changed(number, &listing)?)
        })
        .await?;
        requests.history = history.reads;
        Ok(Selected {
            repository: identity.repository,
            sample,
            history,
            requests,
        })
    }
}

/// Select every repository's sample and history, run its trials, and print
/// every report only once all repositories succeed.
///
/// # Errors
///
/// Returns configuration, sampling, history, and trial failures.
async fn run() -> Result<(), Box<dyn Error>> {
    let token = env::var("GITHUB_TOKEN").map_err(|_| "GITHUB_TOKEN is required")?;
    let client = GitHubClient::new(
        GitHubHost::public(),
        InstallationToken::new(&token)?,
        RequestLimits::default(),
    )?;
    let api = Api {
        http: reqwest::Client::new(),
        token,
        client,
    };
    let mut records = Vec::new();
    for repo in &REPOSITORIES {
        let selected = api.select(repo).await?;
        let mut cache = BTreeMap::new();
        let mut trials = Vec::new();
        for sampled in &selected.sample {
            trials.extend(
                api.trials(&selected.repository, sampled, &mut cache)
                    .await?,
            );
        }
        records.push(record(repo, &selected, &trials));
    }
    for record in records {
        println!("{}", serde_json::to_string_pretty(&record)?);
    }
    Ok(())
}

/// `repo`'s source-free JSON report of `selected` and its `trials`.
fn record(repo: &Repository, selected: &Selected, trials: &[Trial]) -> serde_json::Value {
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
    let requests = selected.requests;
    json!({
        "repository": repo.name,
        "cutoff": repo.cutoff,
        "cold_point": selected.history.cold_point,
        "command": "mise run live:context-relevance",
        "requests": {
            "production": {
                "cold_build": requests.listing_pages + requests.history.cold,
                "incremental_update": requests.history.incremental,
                "review": 0,
            },
            "evaluation_only": {
                "base_commit_timestamps": requests.timestamp_reads,
            },
        },
        "token_estimate": TOKEN_ESTIMATE,
        "candidate_allowance": CANDIDATE_ALLOWANCE,
        "prompt_reserve": PROMPT_RESERVE,
        "production_max_candidate_files": PrContextLimits::default().max_candidate_files,
        "df_ceiling": DF_CEILING,
        "rarity_rule": RARITY_RULE,
        "sample": prs,
        "trials": trials,
        "summary": {
            "all": report::summarize(trials.iter()),
            "non_test": report::summarize(trials.iter().filter(|trial| !trial.test_file)),
        },
        "misses": report::misses(trials),
    })
}

/// Entry point: prints the samples, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("context_relevance failed: {error}");
        std::process::exit(1);
    }
}

/// Command contract checks that need no provider.
#[cfg(test)]
mod tests {
    use super::*;

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
