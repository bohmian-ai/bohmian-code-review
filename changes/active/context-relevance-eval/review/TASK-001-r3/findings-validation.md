# TASK-001 round 3: independent finding validation

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..c88bea58b62df3854b28a082fc7dd969ed8cfd6a`. Authority: approved `spec.md` revision 4, original TASK-001, R1 and R2 remediation tasks, `AGENTS.md`, and `architecture/agent-rules.md`. The citation check accepted both Wave 1 claims. I reviewed the cumulative diff, complete affected function bodies, producers and callers, and the claim ledger without Wave 1 reasoning or verdicts. The focused sampler command passed seven tests; none covers the two cases below.

## Validated blocking ledger

### FIND-TASK-001-6 — Merge time after update time is accepted

- **Source/status/gate/classification:** `TASK-B-R3-1` — **CONFIRMED**; task contract (INV-003 provider inconsistency fails a run, REQ-002 deterministic ordering); **INCORRECT**.
- **Root cause and defect class:** `Discovery::offer` at `sample.rs:160-188` validates each timestamp as an instant but never checks the relation `merged_at <= updated_at`. The module's discovery invariant at `sample.rs:3-5` depends on that relation, so individually valid strings can make an inconsistent provider record appear usable.
- **Every sibling/sink:** `Discovery::settled` at `sample.rs:194-201` releases pending PRs using merge time against the oldest update time read. `Api::select` at `main.rs:219-239` computes that boundary from decoded `ListedPr` records, calls `offer`, then qualifies released PRs and can return immediately at 50. The direct `Api::get` at `main.rs:80-94` is the sole producer of these wire records for `select`; the existing test fixture `listed` at `sample.rs:383-395` is the test producer. Both timestamps use fixed UTC format, so lexical comparison is valid after `is_timestamp`.
- **Evidence and reproduction:** With cutoff `2026-01-10T00:00:00Z`, `Discovery::offer(listed(7, "2026-01-05T00:00:00Z", Some("2026-01-06T00:00:00Z")))` reaches `pending`: number, timestamps, branch, cutoff, and commits all pass existing checks at `sample.rs:163-187`. This is a contradictory listed PR, yet `offer` returns `Ok(())`, and `settled(None)` releases it. The source-path reproduction covers the public command because `Api::select` passes each decoded listing record directly to `offer`. The focused `sample::` command passed seven tests; its timestamp test checks each instant, not their relationship.
- **Observable consequence:** An inconsistent listed PR can be qualified and included in the 50-PR sample instead of failing the run. The settlement proof also relies on the same unchecked relation. This violates INV-003 even if a particular input happens to produce the right order.
- **Smallest safe correction:** At the existing `Discovery::offer` boundary, after validating the two timestamps and before filtering by branch or cutoff, reject `merged_at > listed.updated_at` through `SampleError::UnusablePr`. Preserve the fixed UTC validator, equal timestamps, unmerged PR behavior, sorting, and `Api::select` pagination. Do not add a second provider request or reconciliation path.
- **Focused closure proof:** A no-network check supplies a merged PR with valid instants but merge later than update and observes `UnusablePr`; equal and later update times still enter discovery and retain settlement order. Keep the existing seven sampler tests and run the owner, format, and lint lanes.

## Prior finding closure

- **FIND-TASK-001-1 — closed:** `Listing::push` at `sample.rs:257-271` counts accepted and rejected entries and marks the 3,000-entry boundary incomplete; `Screen::new` fails it at `:321-323`. The cap test passed.
- **FIND-TASK-001-2 — closed:** New test items have rustdoc, including `mod tests` at `sample.rs:376-379` and `PATCH` at `:418-419`.
- **FIND-TASK-001-3 — closed:** `Listing::push` rejects a repeated canonical path at `sample.rs:258-266`; the within-page and across-page test passed.
- **FIND-TASK-001-4 — closed:** `is_timestamp` validates Gregorian date and clock at `sample.rs:208-232` and is used for cutoff, update, and merge ingress; its focused test passed. The cross-field relation in FIND-TASK-001-6 is a separate class from malformed individual instants.
- **FIND-TASK-001-5 — closed:** `listing`, `base_snapshot`, `qualify`, and `select` are inherent `Api` methods at `main.rs:74-249`; `run` and `main` remain entry points.

## Notes, rejections, and validation result

`TASK-A-1` is **REJECTED**: the cited `Some(Ok(ranges))` arm accepts an empty vector and leaves `omitted` unchanged. No note remains. `TASK-B-R3-1` is the one retained blocker and receives the next unused stable ID, `FIND-TASK-001-6`. No prior `FIND-*` class is incomplete. The correction is private to the existing discovery owner and needs no spec revision. Live GitHub sampling and rerun comparison remain TASK-003 verification limits.

**Validation: FAIL — one confirmed blocking finding.**
