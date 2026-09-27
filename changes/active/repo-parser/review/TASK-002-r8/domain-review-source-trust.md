# Domain review: GitHub source trust and provider bounds

**Subject:** `ab2fa3a` → `3979b427ed52e298f5fa88ef0c00bb324f0acc6b`; TASK-002 Revision 12 delta, approved spec revision 12. Earlier TASK-002 behavior passed in r7. **Result: PASS.** No material proposed findings.

## Boundary and authority

Reviewed `github.rs` and its `reqwest` transport, the unchanged `GitHubHost` and identity constructors in `identity.rs`, the pipeline and workspace manifests, and the source client's fake-provider tests. The governing obligations are AC-R12-1 and AC-R12-4 in the task, REQ-004 and REQ-013 in the spec, and the pipeline trust boundary in `architecture/bohmian-code-review-design.md`. I compared the complete base-to-candidate `github.rs` diff and traced production consumers in `snapshot.rs`; its cold-build internals are TASK-003's concern.

| Check | Source evidence | Result |
|---|---|---|
| Authenticated API requests; uncredentialed, host-checked archive redirect | `GitHubClient::request`/`build`, `archive`, `is_trusted_redirect`; `ReqwestTransport::new` disables automatic redirects | PASS |
| Bounded JSON/blob/archive bodies and blob identity | `read_bounded`, `Archive::chunk`, `blob` Git hash check; existing async fake cases cover limits and mismatch | PASS |
| Repository, branch, commit, tree, PR identity before return | Existing checks remain in each public operation; async conversion preserves them | PASS |
| Typed rate/retry outcomes and bounded waits | `send_with_policy` retains the shared 403/429, 5xx and transport policy, attempt cap and maximum per-wait duration; waits now use `tokio::time::sleep` | PASS |
| Drop during retry | The sleep belongs to the calling future; `dropped_call_sends_no_further_requests` checks that only one request was sent after timeout and clock advance | PASS |

## Verification limits

This was a read-only domain audit; I ran no cargo commands or live GitHub/GHES requests. Existing fake-provider tests use `HttpTransport` and `FixedBody`, so they do not exercise a real `reqwest` connection, TLS, or response stream. The orchestrator owns build/test verification. No new source-trust regression was established from the diff.

```citations
[]
```
