# Maintainer decision: store clock, FIND-7 withdrawn, FIND-8/9 shape

Decided 2026-09-26. Authority: spec revision 13. This overrides the FIND-8 correction shape in `TASK-005-R4-pr-file-size-and-root-policy.md`; its FIND-9 part stands.

**FIND-TASK-005-7 is invalid.** Remove its `older_than` cutoff from context cleanup.

**The store owns the clock (spec revision 13).** No host timestamp crosses a store trait, and library code never reads the host clock for durable time:
- `upsert_record(identity, key)` — the store stamps `touched_at`.
- `unreferenced(identity, grace: Duration)` — the store evaluates staleness as its own `now() - grace`. `Snapshots::collect(identity, grace)`; `remove_repository` uses zero grace.
- `upsert_context` — the store stamps `published_at` and `expires_at` (store now + 24h) and returns them.
- Context read and `expired_contexts(limit)` — the store decides expiry with its own clock.
- Delete library `SystemTime::now()` calls for durable time and the `now` parameters on `Contexts::publish` / `Contexts::read`.
- Context ID = content digest plus a random value, not host time.
- The in-memory test store owns a manually advanced clock.
- `github.rs` rate-limit wait computation is a timer, not durable time; it stays.

**FIND-8 — no post-fetch size guards.**
- Fetch every PR blob with the byte limit `min(max_file_bytes, provider ceiling)`. An oversize blob is never returned; it is a file-local oversize omission.
- Put `max_file_bytes` in the snapshot key, next to extraction and format versions, so facts built under a different limit can never be reused.

**FIND-9.** Read the root `.gitattributes` for both regular modes (`100644`, `100755`).
