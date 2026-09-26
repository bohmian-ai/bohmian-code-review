# Claim ledger — TASK-002 r4 (input to ponytail-rev)

Subject: base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `801d9572cf6b0364b5ab33e858388d86f54cb91e`.

## Citation-checked findings

None. All five Wave 1 reports (task-rev A, task-rev B, repo-rev, domain-rev source-trust, domain-rev provider-bounds) proposed zero findings; `review:verify-citations` exit 0 with 0 citations.

## Prior FIND-* ledger (for closure assessment)

| ID | r3 status | r3 owner / siblings |
|---|---|---|
| FIND-TASK-002-1 | CLOSED (r3) | `storage.rs` / `identity.rs` key validation, round-trip |
| FIND-TASK-002-2 | CLOSED (r3) | `# Errors` on every fallible item |
| FIND-TASK-002-3 | CLOSED (r3) | `ObjectKind` removed |
| FIND-TASK-002-4 | CLASS_INCOMPLETE at r3 | `identity.rs` `GitHubHost::Enterprise` payload writable via `&mut`; consumers `api_base()`, `archive_host()`, `github.rs` request URL, bearer attach, `is_trusted_redirect` |
| FIND-TASK-002-5 | CONFIRMED at r3 | `mise.toml` `test:pipeline` did not run doctests |
| FIND-TASK-002-6 | CONFIRMED at r3 | `github.rs` `read_bounded` `limit as u64 + 1`; `execution.rs` `with_timeout` undocumented panic |
| FIND-TASK-002-7 | CONFIRMED at r3 | five `mod tests` without rustdoc |
| FIND-TASK-002-8 | CONFIRMED at r3 | `persistence.rs` path-qualified `std::fmt` |

## Non-blocking observations reported by Wave 1 (notes candidates; not claims)

- source-trust O-1: `GitHubError::UntrustedRedirect { location }` echoes the raw `Location` header into error text.
- source-trust O-2: `ApiBase` is not re-exported from `lib.rs`; reachable only as `identity::ApiBase`.
- source-trust O-3: `ObjectDigest` derives plain `Deserialize`; fails closed at integrity check rather than load.
- provider-bounds O-1: `sleep_bounded` slice is `min(left, 100ms)` not also capped by remaining deadline (≤100 ms drift).
- provider-bounds O-2: `max_attempts: 0` behaves as 1 versus the doc wording.
- provider-bounds O-3: `Archive` stream reads do not observe the `CancelToken` (consumer concern).
- repo-rev: private struct fields lack `///` (fields are not items); `std::hint::black_box` path-qualified inside a test body.
