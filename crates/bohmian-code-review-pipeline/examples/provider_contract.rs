//! Controlled-provider contract check: exercise the production GitHub
//! client and transport, read-only, against a controlled repository at an
//! immutable commit, and record every response's status and headers.
//!
//! Configuration comes from the environment:
//!
//! - `GITHUB_TOKEN` (required): a read-only token for the repository.
//! - `BCR_REPO_ID` (required): the repository's numeric id.
//! - `BCR_COMMIT` (required): the immutable commit under test.
//! - `BCR_REPO` (default `bohmian-ai/wyrd`) and `BCR_PR` (default `91`), a
//!   PR whose files include some without a patch.
//!
//! Cases: repository identity, commit-to-tree, recursive tree, verified
//! blob, archive redirect and streamed body, and PR files with omitted
//! patches. The client sends no conditional request, so one raw
//! `If-None-Match` request replays the repository response's `ETag`
//! through the same transport as a provider observation only. Rate-limit
//! headers are recorded from normal responses; 403/429 and GHES handling
//! are proven by the fake-transport tests in `github.rs`, not here. The
//! command prints one JSON report and exits nonzero on any failed case.

use bohmian_code_review_pipeline::{
    BoxFuture, GitHubClient, GitHubHost, HttpRequest, HttpResponse, HttpTransport,
    InstallationToken, ObjectSha, RepositoryIdentity, RequestLimits, ReqwestTransport,
    TransportError, TreeEntryMode,
};
use serde_json::{Value, json};
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Response headers worth recording; none carries a secret.
const RECORDED: [&str; 8] = [
    "content-type",
    "etag",
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset",
    "x-ratelimit-used",
    "x-ratelimit-resource",
    "retry-after",
];

/// Production transport that records each exchange's URL, status, and
/// [`RECORDED`] headers. Redirect locations are reduced to their host and
/// URLs carrying a `token=` query lose the query, so no credential is kept.
struct Recording {
    inner: ReqwestTransport,
    log: Arc<Mutex<Vec<Value>>>,
}

impl HttpTransport for Recording {
    /// Send through the production transport, then record the response.
    ///
    /// # Errors
    ///
    /// The inner transport's error, unrecorded.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        Box::pin(async move {
            let url = redact(&request.url);
            let response = self.inner.get(request).await?;
            let mut headers = serde_json::Map::new();
            for (name, value) in &response.headers {
                if RECORDED.contains(&name.as_str()) {
                    headers.insert(name.clone(), json!(value));
                } else if name == "location" {
                    headers.insert(name.clone(), json!(host(value)));
                }
            }
            self.log
                .lock()
                .expect("log lock")
                .push(json!({"url": url, "status": response.status, "headers": headers}));
            Ok(response)
        })
    }
}

/// `url` without its query when the query carries a token.
fn redact(url: &str) -> String {
    match url.split_once('?') {
        Some((base, query)) if query.contains("token=") => format!("{base}?<redacted>"),
        _ => url.to_owned(),
    }
}

/// Scheme and host of `url`.
fn host(url: &str) -> String {
    url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/")
}

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

/// Run every case and print the report.
///
/// # Errors
///
/// Returns configuration failures and the first failed case.
async fn run() -> Result<(), Box<dyn Error>> {
    let token = setting("GITHUB_TOKEN", None)?;
    let repo_id: u64 = setting("BCR_REPO_ID", None)?.parse()?;
    let commit = ObjectSha::new(&setting("BCR_COMMIT", None)?)?;
    let repo = setting("BCR_REPO", Some("bohmian-ai/wyrd"))?;
    let number: u64 = setting("BCR_PR", Some("91"))?.parse()?;
    let (owner, name) = repo
        .split_once('/')
        .ok_or("BCR_REPO must be <owner>/<name>")?;
    let repository = RepositoryIdentity::new(repo_id, owner, name)?;
    let log = Arc::new(Mutex::new(Vec::new()));
    let client = GitHubClient::with_transport(
        GitHubHost::public(),
        InstallationToken::new(&token)?,
        RequestLimits::default(),
        Box::new(Recording {
            inner: ReqwestTransport::new()?,
            log: Arc::clone(&log),
        }),
    );
    let mut cases = Vec::new();
    let mut case = |name: &str, result: Value| {
        let exchanges = std::mem::take(&mut *log.lock().expect("log lock"));
        cases.push(json!({"case": name, "result": result, "exchanges": exchanges}));
        exchanges_etag(cases.last())
    };

    let meta = client.repository(&repository).await?;
    let etag = case(
        "repository",
        json!({"full_name": meta.full_name, "default_branch": meta.default_branch, "private": meta.private}),
    );
    let pinned = client.commit(&repository, &commit).await?;
    case(
        "commit",
        json!({"commit": pinned.sha.as_str(), "tree": pinned.tree.as_str()}),
    );
    let listing = client.tree(&repository, &pinned.tree).await?;
    let blob = listing
        .entries
        .iter()
        .find(|entry| entry.mode == TreeEntryMode::File)
        .ok_or("tree has no regular file")?;
    case(
        "recursive_tree",
        json!({"entries": listing.entries.len(), "truncated": listing.truncated, "invalid_paths": listing.invalid_paths}),
    );
    let bytes = client.blob(&repository, &blob.sha, 1 << 20).await?;
    case(
        "blob",
        json!({"path": blob.path.as_str(), "sha": blob.sha.as_str(), "bytes": bytes.len(), "verified": true}),
    );
    let mut archive = client.archive(&repository, &commit).await?;
    let (mut gzip_bytes, mut magic) = (0, Vec::new());
    while let Some(chunk) = archive.chunk().await? {
        if magic.is_empty() {
            magic = chunk.iter().take(2).copied().collect();
        }
        gzip_bytes += chunk.len();
    }
    case(
        "archive",
        json!({"gzip_bytes": gzip_bytes, "gzip_magic": magic == [0x1f, 0x8b]}),
    );
    let (mut files, mut without_patch, mut page) = (0, Vec::new(), 1);
    loop {
        let listed = client.pull_request_files(&repository, number, page).await?;
        files += listed.files.len();
        without_patch.extend(
            listed
                .files
                .iter()
                .filter(|file| file.patch.is_none())
                .map(|file| file.path.as_str().to_owned()),
        );
        if !listed.has_next {
            break;
        }
        page += 1;
    }
    case(
        "pull_request_files",
        json!({"pull_request": number, "files": files, "without_patch": without_patch}),
    );
    let etag = etag.ok_or("repository response carried no etag")?;
    let recorder = Recording {
        inner: ReqwestTransport::new()?,
        log: Arc::clone(&log),
    };
    let conditional = recorder
        .get(HttpRequest {
            url: format!("https://api.github.com/repos/{repo}"),
            headers: vec![
                ("authorization", format!("Bearer {token}")),
                ("accept", "application/vnd.github+json".to_owned()),
                ("x-github-api-version", "2022-11-28".to_owned()),
                ("if-none-match", etag.clone()),
            ],
            timeout: Duration::from_secs(30),
        })
        .await?;
    case(
        "conditional_observation",
        json!({"if_none_match": etag, "status": conditional.status, "client_path": false}),
    );

    let report = json!({
        "repository": repo,
        "repository_id": repo_id,
        "commit": commit.as_str(),
        "tree": pinned.tree.as_str(),
        "pull_request": number,
        "cases": cases,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if without_patch.is_empty() {
        return Err("pull request has no file without a patch".into());
    }
    Ok(())
}

/// The first recorded `etag` of `case`'s exchanges.
fn exchanges_etag(case: Option<&Value>) -> Option<String> {
    case?["exchanges"]
        .as_array()?
        .iter()
        .find_map(|exchange| exchange["headers"]["etag"].as_str().map(str::to_owned))
}

/// Entry point: prints the report, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("provider_contract failed: {error}");
        std::process::exit(1);
    }
}
