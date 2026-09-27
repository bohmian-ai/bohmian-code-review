//! S20: build and read back review context for an existing pull request in
//! a real GitHub repository through the public facades.
//!
//! Configuration comes from the environment:
//!
//! - `GITHUB_TOKEN` (required): a read-only token for the repository.
//! - `BCR_REPO_ID` (required): the repository's numeric id, the caller's
//!   verified identity (`gh api repos/<owner>/<name> --jq .id`).
//! - `BCR_REPO` (default `bohmian-ai/wyrd`) and `BCR_PR` (default `91`).
//! - `BCR_TIMEOUT_SECS` (default `1800`): each facade call's own timeout.
//! - `BCR_WARM_RUNS` (default `0`): further PR-context calls over the same
//!   stores, reported as warm p50/p95 for capacity measurements.
//!
//! Storage and persistence are the in-memory test adapters, so nothing is
//! written outside this process and GitHub is only read. The command prints
//! one JSON record of identity, coverage, omissions, selected spans, object
//! and metadata identity, provider calls, and elapsed time, and exits
//! nonzero on missing configuration, any facade failure, empty selected
//! context, or a read-back that differs from the published context. Peak
//! memory is measured by the caller (`mise run live:pr` wraps it in
//! `/usr/bin/time`).

use bohmian_code_review_pipeline::testing::{
    MemoryContextStore, MemorySnapshotStore, MemoryStorage,
};
use bohmian_code_review_pipeline::{
    ColdBuild, ColdBuildLimits, Contexts, GitHubClient, GitHubHost, Identity, InstallationId,
    InstallationToken, PrContext, PrContextLimits, RepositoryIdentity, RequestLimits, TenantId,
};
use serde_json::json;
use std::env;
use std::error::Error;
use std::future::Future;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Read `name`, or `default` when unset.
///
/// # Errors
///
/// Returns a message naming a required variable that is unset.
fn setting(name: &str, default: Option<&str>) -> Result<String, String> {
    env::var(name)
        .ok()
        .or_else(|| default.map(str::to_owned))
        .ok_or_else(|| format!("{name} is required"))
}

/// Await `call` under `limit`, returning its value and elapsed milliseconds.
///
/// # Errors
///
/// Returns the call's own error or a timeout naming `stage`.
async fn bounded<T, E: Error + 'static>(
    stage: &str,
    limit: Duration,
    call: impl Future<Output = Result<T, E>>,
) -> Result<(T, u128), Box<dyn Error>> {
    let started = Instant::now();
    let value = timeout(limit, call)
        .await
        .map_err(|_| format!("{stage} exceeded {limit:?}"))??;
    Ok((value, started.elapsed().as_millis()))
}

/// Run cold build, PR context, and read-back for the configured PR.
///
/// # Errors
///
/// Returns configuration, facade, timeout, empty-context, and read-back
/// mismatch failures.
async fn run() -> Result<(), Box<dyn Error>> {
    let token = InstallationToken::new(&setting("GITHUB_TOKEN", None)?)?;
    let repo_id: u64 = setting("BCR_REPO_ID", None)?.parse()?;
    let repo = setting("BCR_REPO", Some("bohmian-ai/wyrd"))?;
    let number: u64 = setting("BCR_PR", Some("91"))?.parse()?;
    let limit = Duration::from_secs(setting("BCR_TIMEOUT_SECS", Some("1800"))?.parse()?);
    let (owner, name) = repo
        .split_once('/')
        .ok_or("BCR_REPO must be <owner>/<name>")?;
    let identity = Identity {
        tenant: TenantId::new("live")?,
        installation: InstallationId::new(1)?,
        repository: RepositoryIdentity::new(repo_id, owner, name)?,
    };
    let client = GitHubClient::new(GitHubHost::public(), token, RequestLimits::default())?;
    let (snapshot_storage, snapshot_store) =
        (MemoryStorage::default(), MemorySnapshotStore::default());
    let (context_storage, context_store) =
        (MemoryStorage::default(), MemoryContextStore::default());

    let cold = ColdBuild::new(
        &client,
        &snapshot_storage,
        &snapshot_store,
        ColdBuildLimits::default(),
    );
    let (built, cold_ms) = bounded("cold build", limit, cold.run(&identity)).await?;
    let snapshot_bytes: usize = stored_bytes(&snapshot_storage).await?;
    let pr = PrContext::new(
        &client,
        &snapshot_storage,
        &snapshot_store,
        &context_storage,
        &context_store,
        PrContextLimits::default(),
    );
    let (published, pr_ms) = bounded("pr context", limit, pr.run(&identity, number)).await?;
    let mut warm_ms = Vec::new();
    for _ in 0..setting("BCR_WARM_RUNS", Some("0"))?.parse::<usize>()? {
        warm_ms.push(
            bounded("warm pr context", limit, pr.run(&identity, number))
                .await?
                .1,
        );
    }
    warm_ms.sort_unstable();
    let percentile = |p: usize| {
        warm_ms
            .get((warm_ms.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let contexts = Contexts::new(&context_storage, &context_store);
    let (read, read_ms) = bounded(
        "read",
        limit,
        contexts.read(&identity, &published.record.id),
    )
    .await?;

    let context = &published.context;
    let report = json!({
        "pull_request": format!("https://github.com/{repo}/pull/{number}"),
        "base": context.base.as_str(),
        "head": context.head.as_str(),
        "tree": context.tree.as_str(),
        "cold_build": {
            "commit": built.commit.as_str(),
            "tree": built.tree.as_str(),
            "outcome": format!("{:?}", built.outcome),
            "eligible": built.coverage.eligible,
            "indexed": built.coverage.indexed,
            "skipped": serde_json::to_value(&built.coverage.skipped)?,
            "excluded": format!("{:?}", built.coverage.excluded),
            "archive_entries": built.ingest.archive_entries,
            "fallback_blobs": built.ingest.fallback_blobs,
            "archive_error": built.ingest.archive_error,
            "peak_verified_bytes": built.ingest.peak_verified_bytes,
            "snapshot_bytes": snapshot_bytes,
        },
        "coverage": serde_json::to_value(&context.coverage)?,
        "full_coverage": context.coverage.is_full(),
        "not_analyzed": serde_json::to_value(&context.not_analyzed)?,
        "omissions": serde_json::to_value(&context.omissions)?,
        "budget": serde_json::to_value(&context.budget)?,
        "spans": context.spans.iter().map(|span| json!({
            "path": span.path.as_str(), "blob": span.blob.as_str(),
            "start": span.start, "end": span.end, "score": span.score,
            "names": span.names, "tokens": span.tokens,
        })).collect::<Vec<_>>(),
        "context_id": published.record.id.as_str(),
        "object_key": published.record.key.as_str(),
        "object_digest": published.record.digest.as_str(),
        "read_back_equal": read == *context,
        "calls": client.calls().snapshot().iter()
            .map(|(operation, count)| (format!("{operation:?}"), *count))
            .collect::<std::collections::BTreeMap<_, _>>(),
        "elapsed_ms": {"cold_build": cold_ms, "pr_context": pr_ms, "read": read_ms,
            "warm_runs": warm_ms.len(), "warm_p50": percentile(50), "warm_p95": percentile(95)},
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if context.spans.is_empty() {
        return Err("selected context is empty".into());
    }
    if read != *context {
        return Err("read-back differs from the published context".into());
    }
    Ok(())
}

/// Total bytes of the snapshot objects the cold build stored.
///
/// # Errors
///
/// Returns a storage read failure.
async fn stored_bytes(storage: &MemoryStorage) -> Result<usize, Box<dyn Error>> {
    let mut total = 0;
    for key in storage.keys() {
        total += bohmian_code_review_pipeline::ObjectStorage::read(storage, &key)
            .await?
            .len();
    }
    Ok(total)
}

/// Entry point: prints the report, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("live_pr failed: {error}");
        std::process::exit(1);
    }
}
