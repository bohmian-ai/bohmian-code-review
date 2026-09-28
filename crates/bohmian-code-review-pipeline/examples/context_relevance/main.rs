//! Context-relevance evaluation: select each evaluated repository's PR
//! sample through the real GitHub API, read-only.
//!
//! Configuration comes from the environment:
//!
//! - `GITHUB_TOKEN` (required): a read-only token.
//! - `BCR_EVAL_CUTOFF` (required): only PRs merged strictly before this
//!   `YYYY-MM-DDTHH:MM:SSZ` time are sampled.
//!
//! For each repository the command discovers merged default-branch PRs,
//! reads each candidate's file listing once, builds its base commit's
//! snapshot in memory, and keeps the [`SAMPLE_SIZE`] most recently merged
//! qualifying PRs. It prints one JSON record per repository of the cutoff
//! and each sampled PR's commits, changed source paths, and patch
//! omissions, and exits nonzero on any run-level sampling failure. Nothing
//! is written to GitHub and no source leaves memory.

mod sample;
mod trial;

use bohmian_code_review::{EXTRACTION_VERSION, RelativePath, SymbolicContext};
use bohmian_code_review_pipeline::testing::{MemorySnapshotStore, MemoryStorage};
use bohmian_code_review_pipeline::{
    ColdBuild, ColdBuildLimits, ColdBuildOutcome, FileOutcome, GitHubClient, GitHubHost, Identity,
    InstallationId, InstallationToken, RepositoryIdentity, RequestLimits, SNAPSHOT_FORMAT_VERSION,
    Snapshot, SnapshotScope, Snapshots, TenantId,
};
use sample::{
    Discovery, ListedPr, Listing, MergedPr, OmittedReason, SAMPLE_SIZE, SampleError, Screen,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::BTreeSet;
use std::env;
use std::error::Error;

/// Repositories evaluated, by `<owner>/<name>`.
const REPOSITORIES: [&str; 2] = ["pydantic/pydantic-ai", "tokio-rs/tokio"];
/// PRs per discovery page, GitHub's maximum.
const PRS_PER_PAGE: usize = 100;
/// File-listing pages read at most: 3,000 files at 100 per page.
const MAX_FILE_PAGES: u32 = 30;

/// A qualifying PR with the inputs its relevance trials run on.
struct Sampled {
    /// Validated identity and commits.
    pr: MergedPr,
    /// Its Python and Rust files and patch omissions.
    screen: Screen,
    /// Complete declaration facts of its base commit.
    snapshot: Snapshot,
}

/// Wire shape of `GET /repos/{owner}/{repo}`.
#[derive(Deserialize)]
struct RepositoryWire {
    id: u64,
    default_branch: String,
}

/// The read-only sampling command: the pipeline GitHub client, direct REST
/// reads it does not offer, and the sample cutoff.
struct Api {
    /// Transport for direct REST reads.
    http: reqwest::Client,
    /// Read-only token for direct REST reads.
    token: String,
    /// Pipeline client for PR file listings and base snapshots.
    client: GitHubClient,
    /// Only PRs merged strictly before this time are sampled.
    cutoff: String,
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

    /// Read PR `number`'s file listing once, following its pages until
    /// [`Listing::push`] ends it.
    ///
    /// # Errors
    ///
    /// Returns provider errors.
    async fn listing(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
    ) -> Result<Listing, Box<dyn Error>> {
        let mut listing = Listing::default();
        for page in 1..=MAX_FILE_PAGES {
            let read = self
                .client
                .pull_request_files(repository, number, page)
                .await?;
            if !listing.push(read, page == MAX_FILE_PAGES) {
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

    /// Qualify `pr` from one file listing, then from its base snapshot only
    /// when the listing alone could qualify.
    ///
    /// # Errors
    ///
    /// Returns run-level [`SampleError`]s and provider or storage errors.
    async fn qualify(
        &self,
        identity: &Identity,
        symbolic: &SymbolicContext,
        pr: MergedPr,
    ) -> Result<Option<Sampled>, Box<dyn Error>> {
        let listed = self.listing(&identity.repository, pr.number).await?;
        let screen = Screen::new(pr.number, listed, |path| {
            symbolic
                .path_language(path.as_str())
                .expect("a RelativePath is canonical")
        })?;
        if !screen.plausible() {
            return Ok(None);
        }
        let snapshot = self.base_snapshot(identity, &pr).await?;
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
        Ok(Some(Sampled {
            pr,
            screen,
            snapshot,
        }))
    }

    /// Select `repo`'s sample: discover merged PRs newest-updated first and
    /// qualify each as soon as no unread PR can precede it.
    ///
    /// # Errors
    ///
    /// Returns [`SampleError::TooFew`] when the listing ends early, any other
    /// run-level [`SampleError`], and provider or storage errors.
    async fn select(&self, repo: &str) -> Result<Vec<Sampled>, Box<dyn Error>> {
        let (owner, name) = repo
            .split_once('/')
            .ok_or("repository must be <owner>/<name>")?;
        let wire: RepositoryWire = self.get(&format!("/repos/{repo}")).await?;
        let identity = Identity {
            tenant: TenantId::new("eval")?,
            installation: InstallationId::new(1)?,
            repository: RepositoryIdentity::new(wire.id, owner, name)?,
        };
        let symbolic = SymbolicContext::new();
        let mut discovery = Discovery::new(&wire.default_branch, &self.cutoff)?;
        let mut sample = Vec::new();
        for page in 1.. {
            let listed: Vec<ListedPr> = self
                .get(&format!(
                    "/repos/{repo}/pulls?state=closed&base={}&sort=updated&direction=desc&per_page={PRS_PER_PAGE}&page={page}",
                    wire.default_branch
                ))
                .await?;
            let exhausted = listed.len() < PRS_PER_PAGE;
            let read_until = listed.iter().map(|pr| pr.updated_at.clone()).min();
            for pr in listed {
                discovery.offer(pr)?;
            }
            let read_until = if exhausted { None } else { read_until };
            for pr in discovery.settled(read_until.as_deref()) {
                if let Some(sampled) = self.qualify(&identity, &symbolic, pr).await? {
                    sample.push(sampled);
                    if sample.len() == SAMPLE_SIZE {
                        return Ok(sample);
                    }
                }
            }
            if exhausted {
                break;
            }
        }
        Err(SampleError::TooFew {
            found: sample.len(),
        }
        .into())
    }
}

/// Select every repository's sample and print it.
///
/// # Errors
///
/// Returns configuration and sampling failures.
async fn run() -> Result<(), Box<dyn Error>> {
    let token = env::var("GITHUB_TOKEN").map_err(|_| "GITHUB_TOKEN is required")?;
    let cutoff = env::var("BCR_EVAL_CUTOFF").map_err(|_| "BCR_EVAL_CUTOFF is required")?;
    let client = GitHubClient::new(
        GitHubHost::public(),
        InstallationToken::new(&token)?,
        RequestLimits::default(),
    )?;
    let api = Api {
        http: reqwest::Client::new(),
        token,
        client,
        cutoff,
    };
    for repo in REPOSITORIES {
        let sample = api.select(repo).await?;
        let prs: Vec<_> = sample
            .iter()
            .map(|sampled| {
                json!({
                    "number": sampled.pr.number,
                    "merged_at": sampled.pr.merged_at,
                    "base": sampled.pr.base.as_str(),
                    "head": sampled.pr.head.as_str(),
                    "base_tree": sampled.snapshot.tree.as_str(),
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
        let record = json!({ "repository": repo, "cutoff": api.cutoff, "sample": prs });
        println!("{}", serde_json::to_string_pretty(&record)?);
    }
    Ok(())
}

/// Entry point: prints the samples, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("context_relevance failed: {error}");
        std::process::exit(1);
    }
}
