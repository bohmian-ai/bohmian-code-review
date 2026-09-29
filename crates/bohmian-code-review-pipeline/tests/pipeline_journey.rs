//! Public outer-crate journey. The main flow drives [`ReviewPipeline`] alone
//! over one fake GitHub, one in-memory storage, and one in-memory store
//! serving snapshots and contexts, as a consumer does. The stage facades
//! are then exercised separately for the advanced paths: cancellation,
//! scoped fallback, and incremental update accounting.
//!
//! Only public items of `bohmian-code-review-pipeline` and the
//! `bohmian-code-review` facade are used, so this proves a consumer can drive
//! the whole pipeline with its own stores and cancellation, and that nothing
//! keeps running once a future completes or is dropped.

use bohmian_code_review::{ChangedBlob, EXTRACTION_VERSION, LineRange, SymbolicContext};
use bohmian_code_review_pipeline::testing::{
    MemoryContextStore, MemorySnapshotStore, MemoryStorage, MemoryStore, RoutedTransport,
};
use bohmian_code_review_pipeline::{
    BoxFuture, ColdBuild, ColdBuildLimits, ColdBuildOutcome, ContextError, Contexts, Freshness,
    GitHubClient, GitHubHost, HttpRequest, HttpResponse, HttpTransport, Identity, InstallationId,
    InstallationToken, ObjectSha, Operation, PipelineLimits, PrContext, PrContextLimits,
    PrContextResult, RepositoryIdentity, RequestLimits, ReviewPipeline, SNAPSHOT_FORMAT_VERSION,
    Scope, SnapshotPointer, SnapshotScope, SnapshotStore, Snapshots, TenantId, TransportError,
    Update,
};
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::json;
use sha1::{Digest, Sha1};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, timeout};

/// Declares `lookupUser` and `saveUser`.
const USER: &[u8] = b"export interface User {\n  id: string;\n}\n\nexport function lookupUser(id: string): User {\n  return { id };\n}\n\nexport function saveUser(user: User): void {\n  console.log(user.id);\n}\n";
/// Declares `formatUser`.
const FORMAT: &[u8] = b"import { User } from \"./user\";\n\nexport function formatUser(user: User): string {\n  return user.id;\n}\n";
/// The default branch's second `src/format.ts`: `formatUser` gains a prefix.
const FORMAT_V2: &[u8] = b"import { User } from \"./user\";\n\nexport function formatUser(user: User): string {\n  return \"user:\" + user.id;\n}\n";
/// Declares `handleRequest`.
const HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  return lookupUser(id).id;\n}\n";
/// Declares nothing the PR's handler uses; related to it only by co-change.
const NOTES: &[u8] = b"export function unrelated(): number {\n  return 1;\n}\n";
/// The PR's handler: formats the looked-up user.
const PR_HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  return formatUser(lookupUser(id));\n}\n";
/// Patch of [`PR_HANDLER`]: head line 4 added.
const PR_PATCH: &str = "@@ -1,5 +1,5 @@\n import { lookupUser } from \"./user\";\n \n export function handleRequest(id: string): string {\n-  return lookupUser(id).id;\n+  return formatUser(lookupUser(id));\n }";

/// The pull request every journey reviews.
const PR: u64 = 7;
/// The review's own execution allowance, started by the caller.
const REVIEW_ALLOWANCE: Duration = Duration::from_secs(30 * 60);
/// The caller's cold-build allowance.
const BUILD_ALLOWANCE: Duration = Duration::from_secs(15 * 60);

/// Committer time of default-branch commit `c`.
const C_AT: &str = "2026-02-01T12:00:00Z";
/// Committer time of default-branch commit `d`.
const D_AT: &str = "2026-02-02T12:00:00Z";

/// Forty copies of `ch` as an object id.
fn sha(ch: char) -> ObjectSha {
    ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
}

/// Git blob id of `bytes`.
fn blob_sha(bytes: &[u8]) -> ObjectSha {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ObjectSha::new(&hex).expect("blob sha")
}

/// The caller's verified identity for the fake repository.
fn identity() -> Identity {
    Identity {
        tenant: TenantId::new("acme").expect("tenant"),
        installation: InstallationId::new(42).expect("installation"),
        repository: RepositoryIdentity::new(1296269, "octocat", "Hello-World").expect("repo"),
    }
}

/// Gzipped tarball of `files` under GitHub's single top-level directory.
fn tarball(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                format!("octocat-Hello-World-ccccccc/{path}"),
                *bytes,
            )
            .expect("entry");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&builder.into_inner().expect("tar"))
        .expect("gzip");
    gz.finish().expect("gzip finish")
}

/// Fake GitHub for one repository, shared between the test and its client.
struct FakeGitHub(Arc<RoutedTransport>);

impl HttpTransport for FakeGitHub {
    /// Delegates to the shared routed transport.
    ///
    /// # Errors
    ///
    /// Never.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        self.0.get(request)
    }
}

impl FakeGitHub {
    /// A repository whose default branch is at commit `c`, tree `7`, with
    /// no merged pull requests.
    fn new(files: &[(&str, &[u8])]) -> FakeGitHub {
        let github = FakeGitHub(Arc::new(RoutedTransport::default()));
        github.0.json(
            "/repos/octocat/Hello-World",
            r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
        );
        github.merged(&[]);
        github.push((&sha('c'), C_AT), &sha('7'), files);
        github
    }

    /// Route the closed-PR listing of `prs` merged into `main`, each
    /// `(number, merged_at, changed paths)` last updated when it merged,
    /// and each PR's one-page file listing.
    fn merged(&self, prs: &[(u64, &str, &[&str])]) {
        let mut listed = prs.to_vec();
        listed.sort_by(|a, b| b.1.cmp(a.1));
        let listed: Vec<_> = listed
            .iter()
            .map(|(number, at, _)| {
                json!({"number": number, "updated_at": at, "merged_at": at,
                       "base": {"ref": "main", "sha": sha('c').as_str()},
                       "head": {"ref": "topic", "sha": sha('b').as_str()}})
            })
            .collect();
        self.0.json(
            "/pulls?state=closed&base=main&sort=updated&direction=desc&per_page=100&page=1",
            &serde_json::Value::from(listed).to_string(),
        );
        for (number, _, paths) in prs {
            let files: Vec<_> = paths
                .iter()
                .map(|path| {
                    json!({"filename": path, "status": "modified", "sha": sha('e').as_str(),
                                   "patch": "@@ -1 +1 @@\n-a\n+b"})
                })
                .collect();
            self.0.json(
                &format!("/pulls/{number}/files?per_page=100&page=1"),
                &serde_json::Value::from(files).to_string(),
            );
        }
    }

    /// Move the default branch to `commit`, committed at `at`, and `tree`
    /// holding `files`.
    fn push(&self, (commit, at): (&ObjectSha, &str), tree: &ObjectSha, files: &[(&str, &[u8])]) {
        self.0.json(
            "/branches/main",
            &json!({"name": "main", "commit": {"sha": commit.as_str(), "commit": {
                "tree": {"sha": tree.as_str()}, "committer": {"date": at}}}})
            .to_string(),
        );
        self.0.json(
            &format!("/git/commits/{commit}"),
            &json!({"sha": commit.as_str(), "tree": {"sha": tree.as_str()}, "committer": {"date": at}})
                .to_string(),
        );
        self.tree(tree, files);
        self.0
            .route(&format!("/tarball/{commit}"), 200, tarball(files));
    }

    /// Route the recursive listing of `tree` and every file's blob.
    fn tree(&self, tree: &ObjectSha, files: &[(&str, &[u8])]) {
        let entries: Vec<_> = files
            .iter()
            .map(|(path, bytes)| {
                let sha = blob_sha(bytes);
                self.0.route(&format!("/git/blobs/{sha}"), 200, *bytes);
                json!({"path": path, "mode": "100644", "type": "blob", "sha": sha.as_str(), "size": bytes.len()})
            })
            .collect();
        self.0.json(
            &format!("/git/trees/{tree}?recursive=1"),
            &json!({"sha": tree.as_str(), "truncated": false, "tree": entries}).to_string(),
        );
    }

    /// Route the compare of `base` holding `old` with `head` holding `new`,
    /// and the non-recursive listings of `tree`, the head's tree, whose
    /// files all sit one directory deep.
    fn compare(
        &self,
        (base, old): (&ObjectSha, &[(&str, &[u8])]),
        (head, tree, new): (&ObjectSha, &ObjectSha, &[(&str, &[u8])]),
    ) {
        let blob = |files: &[(&str, &[u8])], path: &str| {
            files
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, bytes)| blob_sha(bytes))
        };
        let mut files: Vec<_> = new
            .iter()
            .filter_map(|(path, bytes)| match blob(old, path) {
                None => Some(json!({"filename": path, "status": "added"})),
                Some(sha) if sha != blob_sha(bytes) => {
                    Some(json!({"filename": path, "status": "modified"}))
                }
                Some(_) => None,
            })
            .collect();
        files.extend(
            old.iter()
                .filter(|(path, _)| blob(new, path).is_none())
                .map(|(path, _)| json!({"filename": path, "status": "removed"})),
        );
        self.0.json(
            &format!("/compare/{base}...{head}"),
            &json!({"status": "ahead", "base_commit": {"sha": base.as_str()},
                    "commits": [{"sha": head.as_str()}], "files": files})
            .to_string(),
        );
        let mut dirs: Vec<(&str, Vec<serde_json::Value>)> = Vec::new();
        for (path, bytes) in new {
            let (dir, name) = path.split_once('/').expect("one directory deep");
            let entry = json!({"path": name, "mode": "100644", "type": "blob",
                               "sha": blob_sha(bytes).as_str(), "size": bytes.len()});
            match dirs.iter_mut().find(|(d, _)| *d == dir) {
                Some((_, entries)) => entries.push(entry),
                None => dirs.push((dir, vec![entry])),
            }
        }
        let dir_sha = |dir: &str| blob_sha(format!("{tree}:{dir}").as_bytes());
        let root: Vec<_> = dirs
            .iter()
            .map(|(dir, _)| json!({"path": dir, "mode": "040000", "type": "tree", "sha": dir_sha(dir).as_str()}))
            .collect();
        for (sha, entries) in std::iter::once((tree.clone(), root))
            .chain(dirs.into_iter().map(|(d, e)| (dir_sha(d), e)))
        {
            self.0.json(
                &format!("/git/trees/{sha}"),
                &json!({"sha": sha.as_str(), "truncated": false, "tree": entries}).to_string(),
            );
        }
    }

    /// Route PR [`PR`] based on `base`, whose head commit `h` has tree `8`
    /// holding `files`, changing `src/handler.ts` to [`PR_HANDLER`].
    fn pull_request(&self, base: &ObjectSha, files: &[(&str, &[u8])]) {
        let (head, tree) = (sha('a'), sha('8'));
        self.0.json(
            &format!("/pulls/{PR}"),
            &json!({
                "number": PR, "state": "open", "draft": false, "merged": false,
                "base": {"ref": "main", "sha": base.as_str(), "repo": {"id": 1296269}},
                "head": {"ref": "feature", "sha": head.as_str(), "repo": {"id": 1296269}},
            })
            .to_string(),
        );
        self.0.json(
            &format!("/pulls/{PR}/files?per_page=100&page=1"),
            &json!([{"filename": "src/handler.ts", "status": "modified",
                     "sha": blob_sha(PR_HANDLER).as_str(), "patch": PR_PATCH}])
            .to_string(),
        );
        self.0.json(
            &format!("/git/commits/{head}"),
            &json!({"sha": head.as_str(), "tree": {"sha": tree.as_str()},
                    "committer": {"date": "2026-03-01T00:00:00Z"}})
            .to_string(),
        );
        self.tree(&tree, files);
    }

    /// A client over this fake.
    fn client(&self) -> GitHubClient {
        GitHubClient::with_transport(
            GitHubHost::public(),
            InstallationToken::new("ghs_token").expect("token"),
            RequestLimits::default(),
            Box::new(FakeGitHub(Arc::clone(&self.0))),
        )
    }
}

/// The caller's injected capabilities: separate snapshot and context
/// storage and stores.
#[derive(Default)]
struct Capabilities {
    snapshot_storage: MemoryStorage,
    snapshot_store: MemorySnapshotStore,
    context_storage: MemoryStorage,
    context_store: MemoryContextStore,
}

impl Capabilities {
    /// The cold-build facade.
    fn cold<'a>(
        &'a self,
        client: &'a GitHubClient,
    ) -> ColdBuild<'a, MemoryStorage, MemorySnapshotStore> {
        ColdBuild::new(
            client,
            &self.snapshot_storage,
            &self.snapshot_store,
            ColdBuildLimits::default(),
        )
    }

    /// The update facade.
    fn update<'a>(
        &'a self,
        client: &'a GitHubClient,
    ) -> Update<'a, MemoryStorage, MemorySnapshotStore> {
        Update::new(
            client,
            &self.snapshot_storage,
            &self.snapshot_store,
            ColdBuildLimits::default(),
        )
    }

    /// The PR-context facade.
    fn pr<'a>(
        &'a self,
        client: &'a GitHubClient,
    ) -> PrContext<'a, MemoryStorage, MemorySnapshotStore, MemoryContextStore> {
        PrContext::new(
            client,
            &self.snapshot_storage,
            &self.snapshot_store,
            &self.context_storage,
            &self.context_store,
            PrContextLimits::default(),
        )
    }

    /// The context-read facade.
    fn contexts(&self) -> Contexts<'_, MemoryStorage, MemoryContextStore> {
        Contexts::new(&self.context_storage, &self.context_store)
    }

    /// Build PR [`PR`] under the review's own allowance, returning the
    /// result and the allowance it consumed.
    async fn review(&self, client: &GitHubClient) -> (PrContextResult, Duration) {
        let started = Instant::now();
        let result = timeout(REVIEW_ALLOWANCE, self.pr(client).run(&identity(), PR))
            .await
            .expect("review within its allowance")
            .expect("pr context");
        (result, started.elapsed())
    }
}

/// Default branch files at commit `c`.
fn main_v1() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("src/user.ts", USER),
        ("src/format.ts", FORMAT),
        ("src/handler.ts", HANDLER),
    ]
}

/// Default branch files at commit `d`, after `src/format.ts` changed.
fn main_v2() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("src/user.ts", USER),
        ("src/format.ts", FORMAT_V2),
        ("src/handler.ts", HANDLER),
    ]
}

/// PR head files: `main_v2` with the PR's handler.
fn pr_head() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("src/user.ts", USER),
        ("src/format.ts", FORMAT_V2),
        ("src/handler.ts", PR_HANDLER),
    ]
}

/// From-scratch oracle: candidate paths ranked by a fresh parse of every
/// head file for the PR's changed line.
fn oracle_paths(files: &[(&str, &[u8])]) -> Vec<String> {
    let mut index = SymbolicContext::new();
    for (path, bytes) in files {
        index.upsert(path, bytes).expect("upsert");
    }
    index
        .rank_files(
            &[ChangedBlob {
                path: "src/handler.ts",
                head: PR_HANDLER,
                changed: vec![LineRange { start: 4, end: 4 }],
            }],
            &[],
        )
        .expect("rank")
        .into_iter()
        .map(|ranked| ranked.path.to_string())
        .collect()
}

/// Assert every selected span is its head file's exact lines and the spans
/// follow the from-scratch ranking of `files`.
fn assert_head_spans(result: &PrContextResult, files: &[(&str, &[u8])]) {
    let spans = &result.context.spans;
    assert!(!spans.is_empty());
    let paths: Vec<String> = spans.iter().map(|span| span.path.to_string()).collect();
    assert_eq!(paths, oracle_paths(files));
    for span in spans {
        let (_, bytes) = files
            .iter()
            .find(|(path, _)| *path == span.path.as_str())
            .expect("span path in head");
        let text = std::str::from_utf8(bytes).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        let expected = lines[span.start as usize - 1..span.end as usize].join("\n");
        assert_eq!(span.text.trim_end_matches('\n'), expected);
        assert_eq!(span.blob, blob_sha(bytes));
    }
}

/// Each facade is callable on its own: an update with no pointer cold
/// builds, a PR review with no snapshot uses honest scoped coverage, and a
/// read of an unpublished id fails without substituting content.
#[tokio::test]
async fn each_facade_is_independently_callable() {
    let github = FakeGitHub::new(&main_v1());
    github.pull_request(&sha('c'), &pr_head());
    let client = github.client();

    let alone = Capabilities::default();
    let (scoped, _) = alone.review(&client).await;
    assert_eq!(scoped.context.coverage.scope, Scope::Scoped);
    assert!(!scoped.context.coverage.is_full());
    assert_head_spans(&scoped, &pr_head());

    let fresh = Capabilities::default();
    assert_eq!(
        fresh.contexts().read(&identity(), &scoped.record.id).await,
        Err(ContextError::NotFound {
            id: scoped.record.id.clone()
        })
    );

    let updated = fresh
        .update(&client)
        .run(&identity())
        .await
        .expect("update");
    assert!(matches!(updated.outcome, ColdBuildOutcome::Published(_)));
    assert!(updated.coverage.is_complete());
    let built = Capabilities::default()
        .cold(&client)
        .run(&identity())
        .await
        .expect("cold build");
    assert_eq!(built.outcome, updated.outcome);
}

/// Scenario 1 and AC-002: a cold build dropped by the caller's timeout
/// leaves no pointer, the review then falls back to scoped context without
/// spending its own allowance, and a retried build converges. The branch
/// then advances, an update moves the pointer by fetching only the changed
/// blob, a warm review of the PR fetches no archive and equals its
/// from-scratch oracle with full coverage, and the context reads back by id
/// with no provider calls.
#[tokio::test(start_paused = true)]
async fn cold_update_warm_review_and_read_compose() {
    let github = FakeGitHub::new(&main_v1());
    let client = github.client();
    let caps = Capabilities::default();
    let caller = identity();

    caps.snapshot_store.stall_before_swap();
    assert!(
        timeout(BUILD_ALLOWANCE, caps.cold(&client).run(&caller))
            .await
            .is_err()
    );
    let freshness = caps
        .update(&client)
        .freshness(&caller)
        .await
        .expect("freshness");
    assert!(matches!(freshness, Freshness::Stale { pointer: None, .. }));

    github.pull_request(&sha('c'), &main_v1_with_pr());
    let (fallback, spent) = caps.review(&client).await;
    assert_eq!(spent, Duration::ZERO);
    assert_eq!(fallback.context.coverage.scope, Scope::Scoped);
    assert!(!fallback.context.coverage.is_full());

    let retried = caps
        .cold(&client)
        .run(&caller)
        .await
        .expect("retried build");
    let ColdBuildOutcome::Published(first) = retried.outcome else {
        panic!("retry did not publish: {:?}", retried.outcome);
    };
    assert_eq!(first.tree, sha('7'));

    github.push((&sha('d'), D_AT), &sha('9'), &main_v2());
    github.compare((&sha('c'), &main_v1()), (&sha('d'), &sha('9'), &main_v2()));
    let before = client.calls().snapshot();
    let updated = caps.update(&client).run(&caller).await.expect("update");
    let ColdBuildOutcome::Published(second) = updated.outcome else {
        panic!("update did not publish: {:?}", updated.outcome);
    };
    assert_eq!(second.tree, sha('9'));
    let calls = client.calls();
    assert_eq!(calls.get(Operation::Archive), before[&Operation::Archive]);
    assert_eq!(calls.get(Operation::Compare), 1);
    assert_eq!(
        calls.get(Operation::Tree) - before[&Operation::Tree],
        2,
        "non-recursive root and src listings, no tree re-listing"
    );
    assert_eq!(
        (updated.ingest.fallback_blobs, updated.ingest.reused_files),
        (1, 2)
    );

    github.pull_request(&sha('d'), &pr_head());
    let archives = client.calls().get(Operation::Archive);
    let (warm, _) = caps.review(&client).await;
    assert_eq!(client.calls().get(Operation::Archive), archives);
    let context = &warm.context;
    assert!(context.coverage.is_full(), "{:?}", context.coverage);
    assert_eq!(context.coverage.snapshot_tree, Some(sha('9')));
    assert_eq!(
        (context.base.clone(), context.head.clone()),
        (sha('d'), sha('a'))
    );
    assert!(context.not_analyzed.is_empty());
    assert_head_spans(&warm, &pr_head());
    assert!(
        context
            .spans
            .iter()
            .any(|span| span.path.as_str() == "src/format.ts" && span.text.contains("user:"))
    );

    let calls = client.calls().snapshot();
    for _ in 0..2 {
        assert_eq!(
            caps.contexts().read(&caller, &warm.record.id).await,
            Ok(warm.context.clone())
        );
    }
    assert_eq!(client.calls().snapshot(), calls);
    assert_eq!(caps.snapshot_storage.keys().len(), 2);
}

/// PR head files over `main_v1`: the scoped fallback's pinned head.
fn main_v1_with_pr() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("src/user.ts", USER),
        ("src/format.ts", FORMAT),
        ("src/handler.ts", PR_HANDLER),
    ]
}

/// PR numbers in the history of the snapshot `pointer` names for
/// [`identity`] under default limits.
async fn snapshot_history<S: SnapshotStore>(
    storage: &MemoryStorage,
    store: &S,
    pointer: &SnapshotPointer,
) -> Vec<u64> {
    let scope = SnapshotScope {
        identity: identity(),
        extraction_version: EXTRACTION_VERSION.to_owned(),
        format_version: SNAPSHOT_FORMAT_VERSION,
        max_file_bytes: ColdBuildLimits::default().max_file_bytes,
    };
    Snapshots::new(storage, store)
        .read(&scope, pointer)
        .await
        .expect("snapshot")
        .history
        .prs
        .iter()
        .map(|pr| pr.number)
        .collect()
}

/// The consumer path through [`ReviewPipeline`] only. The first `context`
/// on a repository with no snapshot cold-builds, persisting the PRs merged
/// before commit `c`, then publishes. After PR 5 merges and the branch
/// moves to `d`, `refresh` extends the history incrementally, reading one
/// more listing page and one PR file listing. The second `context` never
/// reads the branch or refreshes, reviews on the refreshed snapshot with
/// full coverage, and packs `src/notes.ts`, which shares no symbol with
/// the change and only co-changed with the handler. `read` returns the
/// published context with no provider calls.
#[tokio::test]
async fn review_pipeline_cold_builds_refreshes_and_ranks_co_change() {
    let mut files = main_v1();
    files.push(("src/notes.ts", NOTES));
    let github = FakeGitHub::new(&files);
    let pair: &[&str] = &["src/handler.ts", "src/notes.ts"];
    github.merged(&[
        (3, "2026-01-20T00:00:00Z", pair),
        (4, "2026-01-21T00:00:00Z", pair),
    ]);
    let mut head = files.clone();
    head[2] = ("src/handler.ts", PR_HANDLER);
    github.pull_request(&sha('c'), &head);
    let client = github.client();
    let storage = MemoryStorage::default();
    let store = MemoryStore::default();
    let pipeline = ReviewPipeline::new(&storage, &store, PipelineLimits::default());
    let caller = identity();

    let first = pipeline
        .context(&client, &caller, PR)
        .await
        .expect("first context");
    assert_eq!(client.calls().get(Operation::Archive), 1);
    assert!(first.context.coverage.is_full(), "{:?}", first.context.coverage);
    assert_eq!(first.context.coverage.snapshot_tree, Some(sha('7')));

    github.merged(&[
        (3, "2026-01-20T00:00:00Z", pair),
        (4, "2026-01-21T00:00:00Z", pair),
        (5, "2026-02-02T00:00:00Z", &["src/user.ts"]),
    ]);
    let mut next = files.clone();
    next[0] = ("src/user.ts", FORMAT_V2);
    github.push((&sha('d'), D_AT), &sha('9'), &next);
    github.compare((&sha('c'), &files), (&sha('d'), &sha('9'), &next));
    let before = client.calls().snapshot();
    let refreshed = pipeline.refresh(&client, &caller).await.expect("refresh");
    let ColdBuildOutcome::Published(pointer) = refreshed.outcome else {
        panic!("refresh did not publish: {:?}", refreshed.outcome);
    };
    assert_eq!(pointer.tree, sha('9'));
    assert_eq!(
        snapshot_history(&storage, &store, &pointer).await,
        [5, 4, 3]
    );
    let calls = client.calls();
    assert_eq!(calls.get(Operation::Archive), before[&Operation::Archive]);
    for operation in [Operation::PullRequests, Operation::PullRequestFiles] {
        assert_eq!(
            calls.get(operation) - before[&operation],
            1,
            "{operation:?}"
        );
    }

    let mut head = next.clone();
    head[2] = ("src/handler.ts", PR_HANDLER);
    github.pull_request(&sha('d'), &head);
    let before = client.calls().snapshot();
    let second = pipeline
        .context(&client, &caller, PR)
        .await
        .expect("second context");
    let calls = client.calls();
    for operation in [Operation::BranchHead, Operation::Archive, Operation::Compare] {
        assert_eq!(calls.get(operation), before[&operation], "{operation:?}");
    }
    let context = &second.context;
    assert!(context.coverage.is_full(), "{:?}", context.coverage);
    assert_eq!(context.coverage.snapshot_tree, Some(sha('9')));
    assert_eq!((&context.base, &context.head), (&sha('d'), &sha('a')));
    assert!(!oracle_paths(&head).contains(&"src/notes.ts".to_owned()));
    assert!(
        context
            .spans
            .iter()
            .any(|span| span.path.as_str() == "src/notes.ts")
    );

    let calls = client.calls().snapshot();
    assert_eq!(
        pipeline.read(&caller, &second.record.id).await,
        Ok(second.context.clone())
    );
    assert_eq!(client.calls().snapshot(), calls);
}
