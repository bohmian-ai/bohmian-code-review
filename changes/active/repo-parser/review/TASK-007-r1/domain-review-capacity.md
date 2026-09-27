# Domain review: capacity and complete snapshots

## Boundary and authority

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b` → `6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`, TASK-007 under approved `SPEC-repo-parser@14` and `review/maintainer-decision-stateless-snapshot.md`. I reviewed complete cold snapshot publication, stateless per-path outcomes, warm reuse, honest PR coverage, resource bounds, and the refreshed 1k/10k/50k evidence. Governing obligations: spec lines 31–35, 62–65, 69, 75–76, 86–88, 95, 116–121; TASK-007 acceptance and implementation evidence; `AGENTS.md` pipeline ownership and verification rules; `architecture/agent-rules.md` source and review rules.

## Source and evidence coverage

- `snapshot.rs`: `Coverage::is_complete`, `Snapshot::decode` and `coverage`, cold `build`/`complete`/`publish`, `Ingest::fallback`/`settle`/`carry`; `github.rs`: recursive tree splitting and archive request; `update.rs`: compare-based changed-path update; `tests/pipeline_journey.rs`: cold, update, warm, read journey; `examples/live_pr.rs` and `mise.toml`: live measurement entrypoint and lane.
- Complete candidate evidence: `s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json`, the superseded `s20-wyrd-pr91.json`, and the refreshed TASK-007 record. The candidate diff in this review range changes evidence and task text only; the pipeline code landed earlier in the cumulative candidate.
- Independent JSON reconciliation: cold eligible = indexed + skipped for all three (1,681 = 1,461 + 220; 14,691 = 12,762 + 1,929; 39,496 = 36,780 + 2,716). Every cold skip is `NoFacts`, every cold outcome is `Published`, and archive fallback is zero. Each report has two archive calls total across cold, first PR and three warm PR runs; the warm journey test separately asserts no archive increment. Snapshot bytes are 7,654,890 / 60,389,385 / 66,083,067. Recorded cold times, warm p50/p95, API totals, and peak RSS appear in the TASK-007 table with the named macOS/Apple-silicon release environment.
- Wyrd and rust PR reports claim full repository and overlay coverage. Vscode reports a complete repository snapshot, `overlay_complete: false`, and `full_coverage: false` with `NotFetched` overlay skips after the documented 300-change ceiling. This is an honest distinction under spec revision 14. All three reports have nonempty selected spans and equal read-back.
- The source keeps only per-path indexed or skipped outcomes in `Snapshot`; exclusion counts are reported by ingest and are absent from the serialized snapshot. `Coverage::is_complete` admits only benign content skips. Publication is gated on completeness, and decoding rejects incomplete snapshots. Tree truncation propagates to an incomplete cold result; archive expansion and fallback limits remain explicit.

## Verification limits

I validated the checked-in JSON and traced the source. I did not rerun credentialed GitHub measurements or independently reproduce peak RSS; the recorded environment, commands, pinned commits, results, and `/usr/bin/time -l` method are the available evidence. The live command pins the current default-branch head, so the same commit can be rerun only while it remains that head; the task record discloses this limitation. The vscode measurement proves complete snapshot capacity, while its stale PR base does not prove a complete PR-head overlay. No new capacity target or hosted shadow-run obligation is inferred.

## Proposed findings

None. The checked evidence supports complete published cold snapshots, warm snapshot reuse, honest overlay degradation, and the required resource measurements within this domain.

**Overall: PASS**

```citations
[]
```
