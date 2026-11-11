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
    BaselineProblem, ChangeKind, ComparisonError, ExclusionReason, PinnedReview, PipelineError,
    PullRequestPins, PullRequestState, Source, UnavailableReason,
};
use bohmian_code_review_pipeline::{
    BoxFuture, ColdBuild, ColdBuildLimits, ColdBuildOutcome, ContextError, Contexts, Freshness,
    GitHubClient, GitHubError, GitHubHost, HttpRequest, HttpResponse, HttpTransport, Identity,
    InstallationId, InstallationToken, ObjectSha, Operation, PipelineLimits, PrContext,
    PrContextLimits, PrContextResult, RepositoryIdentity, RequestLimits, ReviewPipeline,
    SNAPSHOT_FORMAT_VERSION, Scope, SnapshotPointer, SnapshotScope, SnapshotStore, Snapshots,
    TenantId, TransportError, Update,
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
            "/repositories/1296269",
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
                       "base": {"ref": "main", "sha": sha('c').as_str(), "repo": {"id": 1296269}},
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
    assert!(
        first.context.coverage.is_full(),
        "{:?}",
        first.context.coverage
    );
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
    for operation in [
        Operation::BranchHead,
        Operation::Archive,
        Operation::Compare,
    ] {
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

/// A repository keeps its snapshot and contexts whatever `owner/name` the
/// caller passes: requests address it by stable id and the id alone is its
/// identity, so after a rename (here, a name the fake provider has never
/// served) the next `context` reuses the snapshot without another archive
/// download and `read` returns the context published under the old name.
#[tokio::test]
async fn review_pipeline_survives_a_repository_rename() {
    let files = main_v1();
    let github = FakeGitHub::new(&files);
    let mut head = files.clone();
    head[2] = ("src/handler.ts", PR_HANDLER);
    github.pull_request(&sha('c'), &head);
    let client = github.client();
    let storage = MemoryStorage::default();
    let store = MemoryStore::default();
    let pipeline = ReviewPipeline::new(&storage, &store, PipelineLimits::default());

    let first = pipeline
        .context(&client, &identity(), PR)
        .await
        .expect("context before rename");
    assert_eq!(client.calls().get(Operation::Archive), 1);

    let renamed = Identity {
        repository: RepositoryIdentity::new(1296269, "new-owner", "Renamed").expect("repo"),
        ..identity()
    };
    let second = pipeline
        .context(&client, &renamed, PR)
        .await
        .expect("context after rename");
    assert_eq!(client.calls().get(Operation::Archive), 1);
    assert!(
        second.context.coverage.is_full(),
        "{:?}",
        second.context.coverage
    );
    assert_eq!(second.context.coverage.snapshot_tree, Some(sha('7')));
    assert_eq!(
        pipeline.read(&renamed, &first.record.id).await,
        Ok(first.context.clone())
    );
}

/// The fork repository a pinned PR's head commits come from.
const FORK: u64 = 999;
/// File ceiling of the pinned journeys, so [`big`] is oversize.
const PINNED_FILE_BYTES: u64 = 4096;
/// Declares nothing; deleted by the pinned PR.
const OLD: &[u8] = b"export function old(): number {\n  return 0;\n}\n";
/// Not UTF-8 and holding NULs.
const LOGO: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
/// Vendored source.
const VENDORED: &[u8] = b"export function vendored(): number {\n  return 2;\n}\n";

/// Default branch files at commit `c` for the pinned journeys.
fn pinned_base() -> Vec<(&'static str, &'static [u8])> {
    let mut files = main_v1();
    files.push(("src/old.ts", OLD));
    files
}

/// Larger than [`PINNED_FILE_BYTES`].
fn big() -> &'static [u8] {
    Box::leak("// big\n".repeat(700).into_bytes().into_boxed_slice())
}

/// Pinned head `a` files: the PR's handler, `src/format.ts` renamed to
/// `src/fmt.ts`, `src/old.ts` deleted, and added binary, vendored, and
/// oversize files.
fn pinned_head() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("src/user.ts", USER),
        ("src/fmt.ts", FORMAT),
        ("src/handler.ts", PR_HANDLER),
        ("assets/logo.png", LOGO),
        ("vendor/lib.ts", VENDORED),
        ("src/big.ts", big()),
    ]
}

/// Commit `b`, the earlier reviewed head: [`pinned_head`] with the
/// original handler.
fn reviewed_head() -> Vec<(&'static str, &'static [u8])> {
    pinned_head()
        .into_iter()
        .map(|(path, bytes)| match path {
            "src/handler.ts" => (path, HANDLER),
            _ => (path, bytes),
        })
        .collect()
}

/// One compare file entry.
fn change(path: &str, status: &str, bytes: &[u8], patch: Option<&str>) -> serde_json::Value {
    json!({"filename": path, "status": status, "sha": blob_sha(bytes).as_str(), "patch": patch})
}

/// The merge-base→`a` change list of [`pinned_head`] against [`pinned_base`].
fn full_changes() -> Vec<serde_json::Value> {
    let mut renamed = change("src/fmt.ts", "renamed", FORMAT, None);
    renamed["previous_filename"] = json!("src/format.ts");
    vec![
        change("src/handler.ts", "modified", PR_HANDLER, Some(PR_PATCH)),
        renamed,
        change(
            "src/old.ts",
            "removed",
            OLD,
            Some("@@ -1,3 +0,0 @@\n-export function old(): number {\n-  return 0;\n-}"),
        ),
        change("assets/logo.png", "added", LOGO, None),
        change(
            "vendor/lib.ts",
            "added",
            VENDORED,
            Some("@@ -0,0 +1,3 @@\n+export function vendored(): number {\n+  return 2;\n+}"),
        ),
        change(
            "src/big.ts",
            "added",
            big(),
            Some("@@ -0,0 +1,700 @@\n+// big"),
        ),
    ]
}

impl FakeGitHub {
    /// Route commit `commit` with root `tree` holding `files`.
    fn commit(&self, commit: &ObjectSha, tree: &ObjectSha, files: &[(&str, &[u8])]) {
        self.0.json(
            &format!("/git/commits/{commit}"),
            &json!({"sha": commit.as_str(), "tree": {"sha": tree.as_str()},
                    "committer": {"date": "2026-03-01T00:00:00Z"}})
            .to_string(),
        );
        self.tree(tree, files);
    }

    /// Route the three-dot compare of `base` with `head`.
    fn compare_files(
        &self,
        (base, head): (&ObjectSha, &ObjectSha),
        (status, merge_base): (&str, &ObjectSha),
        files: Vec<serde_json::Value>,
    ) {
        let commits: Vec<_> = match status {
            "identical" => Vec::new(),
            _ => vec![json!({"sha": head.as_str()})],
        };
        self.0.json(
            &format!("/compare/{base}...{head}"),
            &json!({"status": status, "base_commit": {"sha": base.as_str()},
                    "merge_base_commit": {"sha": merge_base.as_str()},
                    "commits": commits, "files": files})
            .to_string(),
        );
    }

    /// A repository at [`pinned_base`] whose PR [`PR`] from fork [`FORK`]
    /// has moved its live head to `f`, while the caller pinned head `a`
    /// (tree `8`), with the merge-base comparison routed and commit `b`
    /// (tree `9`) as an earlier reviewed head.
    fn pinned() -> FakeGitHub {
        let github = FakeGitHub::new(&pinned_base());
        github.0.json(
            &format!("/pulls/{PR}"),
            &json!({
                "number": PR, "state": "open", "draft": true, "merged": false,
                "html_url": "https://github.com/octocat/Hello-World/pull/7",
                "title": "Format users", "body": null,
                "base": {"ref": "main", "sha": sha('c').as_str(), "repo": {"id": 1296269}},
                "head": {"ref": "feature", "sha": sha('f').as_str(), "repo": {"id": FORK}},
            })
            .to_string(),
        );
        github.commit(&sha('a'), &sha('8'), &pinned_head());
        github.commit(&sha('b'), &sha('9'), &reviewed_head());
        github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), full_changes());
        github
    }
}

/// The caller's frozen pins for PR [`PR`] at head `a`.
fn pins() -> PullRequestPins {
    PullRequestPins {
        number: PR,
        base: sha('c'),
        head: sha('a'),
        head_repository_id: FORK,
    }
}

/// Pipeline ceilings of the pinned journeys.
fn pinned_limits() -> PipelineLimits {
    PipelineLimits {
        max_file_bytes: PINNED_FILE_BYTES,
        ..PipelineLimits::default()
    }
}

/// Text source of `bytes`.
///
/// # Panics
///
/// Panics if `bytes` is not UTF-8.
fn text(bytes: &[u8]) -> Source {
    Source::Text {
        blob: blob_sha(bytes),
        text: String::from_utf8(bytes.to_vec()).expect("utf8"),
    }
}

/// Every span is the exact text of the pinned head file it names.
///
/// # Panics
///
/// Panics if the context has no spans, a span names a path outside the
/// pinned head, the file is not UTF-8, the span's lines are out of range, or
/// a span's text or blob differs from the pinned file.
fn assert_pinned_spans(review: &PinnedReview) {
    let head = pinned_head();
    assert!(!review.context.context.spans.is_empty());
    for span in &review.context.context.spans {
        let (_, bytes) = head
            .iter()
            .find(|(path, _)| *path == span.path.as_str())
            .expect("span path in pinned head");
        let lines: Vec<&str> = std::str::from_utf8(bytes).expect("utf8").lines().collect();
        let expected = lines[span.start as usize - 1..span.end as usize].join("\n");
        assert_eq!(span.text.trim_end_matches('\n'), expected);
        assert_eq!(span.blob, blob_sha(bytes));
    }
}

/// AC-004: a first review of a fork PR whose live head moved past the
/// pins compares exactly merge base → pinned head, cold then warm. Every
/// change carries verified source or names why it has none, the context
/// is selected at the pins with exact span text, and the warm review
/// reuses the snapshot without another archive or a PR file listing.
///
/// # Panics
///
/// Panics if either review fails or differs from the pins, a change lacks
/// verified source or its reason, a span is not exact pinned text, the
/// stored context does not read back, the warm review differs from the cold
/// one or fetches another archive or PR file listing, or any request names
/// the live head.
#[tokio::test]
async fn pinned_first_review_obeys_the_pins_cold_and_warm() {
    let github = FakeGitHub::pinned();
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());

    let cold = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("cold pinned review");
    assert_eq!(client.calls().get(Operation::Archive), 1);
    let source = &cold.pull_request;
    assert_eq!(
        (source.number, source.head.clone(), source.base.clone()),
        (PR, sha('a'), sha('c'))
    );
    assert_eq!(
        (source.head_repository_id, source.merge_base.clone()),
        (FORK, sha('c'))
    );
    assert_eq!(
        (source.title.as_str(), source.body.as_str(), source.draft),
        ("Format users", "", true)
    );
    assert_eq!(source.url, "https://github.com/octocat/Hello-World/pull/7");
    assert_eq!(source.state, PullRequestState::Open);
    assert_eq!(
        (cold.full.base.clone(), cold.full.head.clone()),
        (sha('c'), sha('a'))
    );
    assert_eq!(cold.active, cold.full);

    let files = &cold.full.files;
    let kinds: Vec<(&str, ChangeKind)> =
        files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(
        kinds,
        vec![
            ("src/handler.ts", ChangeKind::Modified),
            ("src/fmt.ts", ChangeKind::Renamed),
            ("src/old.ts", ChangeKind::Deleted),
            ("assets/logo.png", ChangeKind::Added),
            ("vendor/lib.ts", ChangeKind::Added),
            ("src/big.ts", ChangeKind::Added),
        ]
    );
    let handler = &files[0];
    assert_eq!(
        (handler.before.clone(), handler.after.clone()),
        (Some(text(HANDLER)), Some(text(PR_HANDLER)))
    );
    assert_eq!(handler.hunks.len(), 1);
    assert_eq!(
        (handler.hunks[0].old_start, handler.hunks[0].new_lines),
        (1, 5)
    );
    assert_eq!(handler.hunks[0].patch, PR_PATCH);
    let renamed = &files[1];
    assert_eq!(
        renamed.previous_path.as_ref().map(|p| p.as_str()),
        Some("src/format.ts")
    );
    assert_eq!(
        (renamed.before.clone(), renamed.after.clone()),
        (Some(text(FORMAT)), Some(text(FORMAT)))
    );
    assert!(renamed.hunks.is_empty());
    assert_eq!(
        (files[2].before.clone(), files[2].after.clone()),
        (Some(text(OLD)), None)
    );
    assert_eq!(
        (files[3].before.clone(), files[3].after.clone()),
        (
            None,
            Some(Source::Binary {
                blob: blob_sha(LOGO)
            })
        )
    );
    assert_eq!(
        files[4].after,
        Some(Source::Excluded {
            blob: blob_sha(VENDORED),
            reason: ExclusionReason::VendoredSegment
        })
    );
    assert_eq!(
        files[5].after,
        Some(Source::Unavailable {
            blob: blob_sha(big()),
            reason: UnavailableReason::Oversize {
                limit: PINNED_FILE_BYTES
            }
        })
    );

    let context = &cold.context.context;
    assert_eq!(
        (context.head.clone(), context.base.clone()),
        (sha('a'), sha('c'))
    );
    assert_eq!(context.pull_request, PR);
    assert_pinned_spans(&cold);
    assert_eq!(
        pipeline
            .read(&identity(), &cold.context.record.id)
            .await
            .expect("read back"),
        *context
    );

    let warm = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("warm pinned review");
    assert_eq!(client.calls().get(Operation::Archive), 1);
    assert_eq!(client.calls().get(Operation::PullRequestFiles), 0);
    assert_eq!(
        (&warm.full, &warm.pull_request),
        (&cold.full, &cold.pull_request)
    );
    assert_eq!(warm.context.context.spans, cold.context.context.spans);
    assert!(
        github
            .0
            .seen()
            .iter()
            .all(|url| !url.contains(&sha('f').to_string()))
    );
}

/// AC-004: an incremental review compares the earlier reviewed head with
/// the pinned head and selects context for those changes only, keeping the
/// full merge-base comparison beside it; an identical baseline is a valid
/// empty active comparison.
///
/// # Panics
///
/// Panics if a review fails, the active comparison is not baseline → head,
/// the full comparison does not start at the merge base, a span is not exact
/// pinned text, context covers files outside the active changes, or an empty
/// active comparison selects context.
#[tokio::test]
async fn pinned_incremental_review_compares_from_the_baseline() {
    let github = FakeGitHub::pinned();
    github.compare_files(
        (&sha('b'), &sha('a')),
        ("ahead", &sha('b')),
        vec![change(
            "src/handler.ts",
            "modified",
            PR_HANDLER,
            Some(PR_PATCH),
        )],
    );
    github.compare_files((&sha('a'), &sha('a')), ("identical", &sha('a')), Vec::new());
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());

    let review = pipeline
        .pinned(&client, &identity(), &pins(), Some(&sha('b')))
        .await
        .expect("incremental review");
    assert_eq!(
        (review.active.base.clone(), review.active.head.clone()),
        (sha('b'), sha('a'))
    );
    assert_eq!(review.active.files.len(), 1);
    assert_eq!(review.active.files[0].after, Some(text(PR_HANDLER)));
    assert_eq!(review.full.base, sha('c'));
    assert_eq!(review.full.files.len(), 6);
    assert_eq!(review.pull_request.merge_base, sha('c'));
    assert_pinned_spans(&review);
    let full = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("full review");
    assert!(
        full.context
            .context
            .not_analyzed
            .iter()
            .any(|skipped| skipped.path != "src/handler.ts"),
        "{:?}",
        full.context.context.not_analyzed
    );
    assert!(
        review
            .context
            .context
            .not_analyzed
            .iter()
            .all(|skipped| skipped.path == "src/handler.ts"),
        "context is selected for the active changes only: {:?}",
        review.context.context.not_analyzed
    );

    let empty = pipeline
        .pinned(&client, &identity(), &pins(), Some(&sha('a')))
        .await
        .expect("empty active comparison");
    assert!(empty.active.files.is_empty());
    assert_eq!(empty.full.files.len(), 6);
    assert!(empty.context.context.spans.is_empty());
}

/// AC-004: a baseline the head no longer descends from, or one the
/// provider cannot find, is reported unusable and never replaced; a
/// mismatched fork, a truncated comparison, and a text change without a
/// patch fail explicitly; and context-budget omissions stay coverage data.
///
/// # Panics
///
/// Panics if a refused comparison returns another error than the expected
/// comparison error, or a review whose omissions are coverage fails or
/// selects spans for them.
#[tokio::test]
async fn pinned_reviews_keep_limits_and_failures_explicit() {
    let github = FakeGitHub::pinned();
    github.compare_files((&sha('e'), &sha('a')), ("diverged", &sha('c')), Vec::new());
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());
    let comparison = |result: Result<PinnedReview, PipelineError>| match result {
        Err(PipelineError::Comparison(error)) => error,
        other => panic!("expected a comparison error, got {other:?}"),
    };

    for (baseline, problem) in [
        (sha('e'), BaselineProblem::Rewritten),
        (sha('d'), BaselineProblem::Unavailable),
    ] {
        assert_eq!(
            comparison(
                pipeline
                    .pinned(&client, &identity(), &pins(), Some(&baseline))
                    .await
            ),
            ComparisonError::BaselineUnusable { baseline, problem }
        );
    }

    let foreign = PullRequestPins {
        head_repository_id: 5,
        ..pins()
    };
    assert_eq!(
        comparison(pipeline.pinned(&client, &identity(), &foreign, None).await),
        ComparisonError::HeadRepositoryMismatch {
            pinned: 5,
            actual: FORK
        }
    );

    let mut unpatched = full_changes();
    unpatched[0]["patch"] = serde_json::Value::Null;
    github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), unpatched);
    assert!(matches!(
        comparison(pipeline.pinned(&client, &identity(), &pins(), None).await),
        ComparisonError::PatchUnavailable { path } if path.as_str() == "src/handler.ts"
    ));

    let many: Vec<_> = (0..300)
        .map(|i| change(&format!("src/f{i}.ts"), "added", OLD, None))
        .collect();
    github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), many);
    assert_eq!(
        comparison(pipeline.pinned(&client, &identity(), &pins(), None).await),
        ComparisonError::Truncated {
            base: sha('c'),
            head: sha('a')
        }
    );

    github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), full_changes());
    let mut tight = pinned_limits();
    tight.pr.context_tokens = 1;
    let review = ReviewPipeline::new(&storage, &store, tight)
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("omissions are coverage, not failure");
    let context = &review.context.context;
    assert!(context.spans.is_empty());
    assert!(!context.omissions.is_empty());
    assert_eq!(review.full.files.len(), 6);

    let mut misreported = full_changes();
    misreported[0]["sha"] = json!(sha('9').as_str());
    github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), misreported);
    assert!(matches!(
        comparison(pipeline.pinned(&client, &identity(), &pins(), None).await),
        ComparisonError::Unverified { path, .. } if path.as_str() == "src/handler.ts"
    ));

    github.compare_files((&sha('c'), &sha('a')), ("ahead", &sha('c')), full_changes());
    github.0.json(
        &format!("/git/trees/{}?recursive=1", sha('8')),
        &json!({"sha": sha('8').as_str(), "truncated": true, "tree": []}).to_string(),
    );
    github.0.json(
        &format!("/git/trees/{}", sha('8')),
        &json!({"sha": sha('8').as_str(), "truncated": true, "tree": []}).to_string(),
    );
    assert_eq!(
        comparison(pipeline.pinned(&client, &identity(), &pins(), None).await),
        ComparisonError::Truncated {
            base: sha('a'),
            head: sha('a')
        }
    );
}

/// AC-004: when the base branch has moved past the merge base, the full
/// comparison and its sources run from the merge base GitHub reports, never
/// from the pinned base commit itself.
///
/// # Panics
///
/// Panics if either review fails, or a review whose PR base moved past the
/// merge base compares differently from one based at the merge base.
#[tokio::test]
async fn pinned_full_review_compares_from_the_merge_base() {
    let github = FakeGitHub::pinned();
    github.compare_files(
        (&sha('5'), &sha('a')),
        ("diverged", &sha('c')),
        full_changes(),
    );
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());
    let moved = PullRequestPins {
        base: sha('5'),
        ..pins()
    };

    let review = pipeline
        .pinned(&client, &identity(), &moved, None)
        .await
        .expect("review from the merge base");
    let source = &review.pull_request;
    assert_eq!(
        (source.base.clone(), source.merge_base.clone()),
        (sha('5'), sha('c'))
    );
    assert_eq!(
        (review.full.base.clone(), review.full.head.clone()),
        (sha('c'), sha('a'))
    );
    let from_base = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("review with the base at the merge base");
    assert_eq!(review.full.files, from_base.full.files);
    assert_eq!(review.active, review.full);
}

/// AC-004: a pinned head whose recursive tree listing GitHub truncated is
/// rebuilt from its subtrees' listings, so the review matches the one over
/// the untruncated listing instead of failing or guessing.
///
/// # Panics
///
/// Panics if either review fails, or the rebuilt listing yields different
/// comparisons or spans that are not exact pinned text.
#[tokio::test]
async fn pinned_review_rebuilds_a_truncated_head_listing() {
    let github = FakeGitHub::pinned();
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());
    let expected = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("review over the untruncated listing");

    let mut root = Vec::new();
    for (dir, subtree) in [
        ("assets", sha('1')),
        ("src", sha('2')),
        ("vendor", sha('3')),
    ] {
        let entries: Vec<_> = pinned_head()
            .into_iter()
            .filter_map(|(path, bytes)| {
                let name = path.strip_prefix(dir)?.strip_prefix('/')?;
                Some(json!({"path": name, "mode": "100644", "type": "blob",
                            "sha": blob_sha(bytes).as_str(), "size": bytes.len()}))
            })
            .collect();
        github.0.json(
            &format!("/git/trees/{subtree}?recursive=1"),
            &json!({"sha": subtree.as_str(), "truncated": false, "tree": entries}).to_string(),
        );
        root.push(json!({"path": dir, "mode": "040000", "type": "tree", "sha": subtree.as_str()}));
    }
    github.0.json(
        &format!("/git/trees/{}?recursive=1", sha('8')),
        &json!({"sha": sha('8').as_str(), "truncated": true, "tree": []}).to_string(),
    );
    github.0.json(
        &format!("/git/trees/{}", sha('8')),
        &json!({"sha": sha('8').as_str(), "truncated": false, "tree": root}).to_string(),
    );

    let rebuilt = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("review over the rebuilt listing");
    assert_eq!(rebuilt.full, expected.full);
    assert_eq!(rebuilt.active, expected.active);
    assert_pinned_spans(&rebuilt);
}

/// AC-004: a comparison whose commit list fills the provider's first page
/// is accepted on its verified files, since its last listed commit need not
/// be the head; a shorter list that ends anywhere but the pinned head is
/// refused instead of compared.
///
/// # Panics
///
/// Panics if either review fails, a full commit page yields a different
/// comparison, or a short page ending elsewhere is not refused as an
/// identity mismatch.
#[tokio::test]
async fn pinned_reviews_verify_a_paged_comparison() {
    let github = FakeGitHub::pinned();
    let client = github.client();
    let (storage, store) = (MemoryStorage::default(), MemoryStore::default());
    let pipeline = ReviewPipeline::new(&storage, &store, pinned_limits());
    let expected = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("review over a one-page comparison");
    let route = |commits: Vec<serde_json::Value>| {
        github.0.json(
            &format!("/compare/{}...{}", sha('c'), sha('a')),
            &json!({"status": "ahead", "base_commit": {"sha": sha('c').as_str()},
                    "merge_base_commit": {"sha": sha('c').as_str()},
                    "commits": commits, "files": full_changes()})
            .to_string(),
        );
    };

    route(vec![json!({"sha": sha('b').as_str()}); 250]);
    let paged = pipeline
        .pinned(&client, &identity(), &pins(), None)
        .await
        .expect("a paged commit list is verified by its trees");
    assert_eq!(paged.full, expected.full);

    route(vec![json!({"sha": sha('b').as_str()})]);
    assert!(matches!(
        pipeline.pinned(&client, &identity(), &pins(), None).await,
        Err(PipelineError::Comparison(ComparisonError::Provider(
            GitHubError::IdentityMismatch { .. }
        )))
    ));
}
