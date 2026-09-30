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
//! command prints one JSON report and exits nonzero on any failed case:
//! [`failures`] requires every named case, a complete untruncated tree,
//! an archive redirected to a gzip body, a PR file without a patch, a 304
//! conditional observation, and rate headers on every API response.

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

/// Cases a passing report must contain.
const REQUIRED: [&str; 7] = [
    "repository",
    "commit",
    "recursive_tree",
    "blob",
    "archive",
    "pull_request_files",
    "conditional_observation",
];

/// Rate-limit headers every `api.github.com` response must carry.
const RATE_HEADERS: [&str; 5] = [
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset",
    "x-ratelimit-used",
    "x-ratelimit-resource",
];

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
            url: format!("https://api.github.com/repositories/{repo_id}"),
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
    let failed = failures(&cases);
    if !failed.is_empty() {
        return Err(failed.join("; ").into());
    }
    Ok(())
}

/// Every failed named case of `cases`, empty when the contract holds.
fn failures(cases: &[Value]) -> Vec<String> {
    let mut failed: Vec<String> = REQUIRED
        .iter()
        .filter(|name| !cases.iter().any(|case| case["case"] == **name))
        .map(|name| format!("{name}: case missing"))
        .collect();
    for case in cases {
        let (name, result) = (case["case"].as_str().unwrap_or(""), &case["result"]);
        let exchanges = case["exchanges"].as_array().map_or(&[][..], Vec::as_slice);
        let problem = match name {
            "recursive_tree"
                if result["entries"].as_u64().unwrap_or(0) == 0
                    || result["truncated"] != false
                    || result["invalid_paths"] != 0 =>
            {
                Some("tree empty, truncated, or with invalid paths")
            }
            "archive"
                if exchanges.first().map(|e| &e["status"]) != Some(&json!(302))
                    || exchanges
                        .first()
                        .map(|e| &e["headers"]["location"])
                        .is_none_or(Value::is_null)
                    || exchanges.last().map(|e| &e["status"]) != Some(&json!(200))
                    || result["gzip_magic"] != true =>
            {
                Some("archive not a redirect to a gzip body")
            }
            "pull_request_files"
                if result["without_patch"].as_array().is_none_or(Vec::is_empty) =>
            {
                Some("no file without a patch")
            }
            "conditional_observation" if result["status"] != 304 => {
                Some("conditional replay did not return 304")
            }
            _ => None,
        };
        failed.extend(problem.map(|problem| format!("{name}: {problem}")));
        let unrated = exchanges.iter().any(|exchange| {
            exchange["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("https://api.github.com/"))
                && RATE_HEADERS
                    .iter()
                    .any(|header| exchange["headers"][header].is_null())
        });
        if unrated {
            failed.push(format!("{name}: API response without rate headers"));
        }
    }
    failed
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

#[cfg(test)]
mod tests {
    //! Offline checks of the provider verdict against compact case data
    //! captured from the controlled Wyrd run and owned by this crate: the
    //! recorded passing cases stay passing, and breaking or
    //! removing any single named case makes `failures` report that case, so
    //! the command's exit status cannot pass on a partially failed run.

    use super::*;

    /// Cases of a captured controlled-provider report, reduced to the
    /// fields [`failures`] inspects.
    const CAPTURED: &str = include_str!("provider_contract_cases.json");

    /// Cases of the captured report.
    fn captured() -> Vec<Value> {
        let report: Value = serde_json::from_str(CAPTURED).expect("report");
        report["cases"].as_array().expect("cases").clone()
    }

    /// The captured passing report passes, and each failed named case in
    /// turn, or a missing one, fails it.
    #[test]
    fn every_named_case_decides_the_verdict() {
        assert_eq!(failures(&captured()), Vec::<String>::new());
        let broken: [(&str, fn(&mut Value)); 9] = [
            ("recursive_tree", |c| c["result"]["truncated"] = json!(true)),
            ("recursive_tree", |c| {
                c["result"]["invalid_paths"] = json!(2)
            }),
            ("archive", |c| c["result"]["gzip_magic"] = json!(false)),
            ("archive", |c| c["exchanges"][0]["status"] = json!(200)),
            ("pull_request_files", |c| {
                c["result"]["without_patch"] = json!([])
            }),
            ("conditional_observation", |c| {
                c["result"]["status"] = json!(200)
            }),
            ("blob", |c| {
                c["exchanges"][0]["headers"]
                    .as_object_mut()
                    .expect("headers")
                    .remove("x-ratelimit-remaining");
            }),
            ("commit", |c| *c = json!({"case": "other"})),
            ("repository", |c| {
                c["exchanges"][0]["headers"]
                    .as_object_mut()
                    .expect("headers")
                    .remove("x-ratelimit-resource");
            }),
        ];
        for (name, breaks) in broken {
            let mut cases = captured();
            let case = cases
                .iter_mut()
                .find(|case| case["case"] == name)
                .expect("case");
            breaks(case);
            let failed = failures(&cases);
            assert!(
                failed.iter().any(|failure| failure.starts_with(name)),
                "{name}: {failed:?}"
            );
        }
    }
}
