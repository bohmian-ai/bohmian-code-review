# TASK-001 round 4: independent finding validation

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original TASK-001, R1–R3 remediation tasks, `AGENTS.md`, and applicable architecture rules. I inspected the complete cumulative diff, the citation-checked R4 claim ledger, the prior finding ledger, and the full affected source and caller bodies. I did not use Wave 1 evidence prose, matrices, or verdicts.

## Validated blocking ledger

### FIND-TASK-001-7 — A patch without a hunk is accepted as a non-omission

- **Source/status/gate/classification:** `SRC-R4-1` — **CONFIRMED**; task contract (REQ-002, TASK-001 scenario 2, and the explicit missing-or-malformed-patch omission rule); **INCORRECT**. This is a new class, not a recurrence of a prior `FIND-*`.
- **Root cause/defect class:** `Screen::new` at `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:345-355` treats every `Ok(Vec<LineRange>)` from the public hunk parser as a valid patch. `parse_hunks` at `crates/bohmian-code-review-select/src/diff.rs:137-149,243-247` accepts an empty string or only a `\\` marker and returns `Ok([])` because it never requires a hunk to start. At the sampler boundary, patch presence has therefore been confused with patch validity. The parser is production code and need not change for this evaluation-only obligation.
- **Every sibling and sink:** Empty and marker-only `Some(patch)` values enter the same `Screen::new` match. They set `changed_lines` false but leave `Screen::omitted` empty. `Screen::plausible` and `Screen::qualifies` at `sample.rs:363-379` use that flag; `Api::qualify` at `examples/context_relevance/main.rs:165-197` is the only command caller and can retain a PR with another valid changed-line file. `Api::select` at `main.rs:206-248` can include it in the sample; `run` at `main.rs:270-295` prints the omission list. The sole provider producer is `Api::listing` at `main.rs:102-118`, which folds `GitHubClient::pull_request_files` pages into `Listing`; the `file` fixture at `sample.rs:403-411` is the test producer. The correction need not touch the valid, zero-added-line deletion hunk: `parse_hunks("@@ -4,1 +3,0 @@\n-gone\n") == Ok([])` is covered at `diff.rs:620`.
- **Reproduction:** A listed eligible `src/b.py` with `patch: Some("")`, or `Some("\\ No newline at end of file")`, reaches `Screen::new`. `parse_hunks` starts with default `Hunks`, consumes no hunk, and `finish` returns `Ok([])`. The `Some(Ok(ranges))` arm records no omission. Pair it with a modified base-declared `src/a.py` carrying a valid changed-line hunk: the PR qualifies, and the emitted `omitted` list lacks `src/b.py`. The exact focused sampler command, `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::`, passes 8 tests; its omission test covers `None` and a non-hunk text that `parse_hunks` rejects, but not these accepted no-hunk inputs.
- **Observable consequence:** A qualifying PR's report silently understates patch omissions. TASK-002 would receive a listed file with unusable patch text as though its patch were valid; the changed-lines flag alone does not satisfy the required omission record.
- **Decision-complete correction:** At the existing `Screen::new` patch classification boundary, require a present patch to contain a successfully parsed hunk before treating it as valid. Record present no-hunk input through `OmittedReason::MalformedPatch` and keep its changed-lines flag false. Preserve existing behavior for `None`, malformed hunks, valid added-line hunks, and valid deletion-only hunks. Reuse `Screen::omitted`, `OmittedReason`, and `parse_hunks`; do not alter the public parser, production behavior, or provider flow.
- **Focused closure proof:** Extend the existing no-network screen test with `Some("")` and marker-only patches and assert both are omitted as malformed, contribute no changed lines, and appear in the emitted omission path. Assert a valid deletion-only hunk is *not* omitted and also contributes no head-side changed lines. Run the exact sampler selector and required owner, format, lint, and diff checks.

## Prior finding closure and redundancy audit

- **FIND-TASK-001-1 — closed:** `Listing::push` counts accepted plus rejected entries against the provider ceiling at `sample.rs:263-277`; `Screen::new` rejects truncation at `:327-329`.
- **FIND-TASK-001-2 — closed:** The test module and its `PATCH` fixture have rustdoc at `sample.rs:382-425`.
- **FIND-TASK-001-3 — closed:** `Listing::push` rejects duplicate canonical paths at `sample.rs:264-274`.
- **FIND-TASK-001-4 — closed:** `is_timestamp` checks Gregorian date and clock at `sample.rs:211-239`, and both cutoff and listed timestamps use it at `:140-142,169-176`.
- **FIND-TASK-001-5 — closed:** The stateful listing, snapshot, qualification, and selection workflow belongs to concrete `Api` at `main.rs:74-249`.
- **FIND-TASK-001-6 — closed:** `Discovery::offer` rejects merge time after update time at `sample.rs:175-183`, before filtering or queuing.

No R1–R3 remediation code is rendered redundant by the required no-hunk classification. The R3 Wave 2 ledger rejected the same empty-range symptom; source inspection here distinguishes valid deletion-only hunks from inputs with no hunk and establishes the named omission obligation.

## Rejections, notes, and limits

The citation verifier rejected `DOMAIN-SAMPLING-1`, `DOMAIN-SAMPLING-2`, and `TASK-B-1`; they are excluded rather than downgraded. No retained notes. No network-backed sample or rerun was attempted; those belong to TASK-003. The current focused tests pass but do not exercise the reproduced no-hunk path.

**Validation: FAIL — one confirmed blocking finding, `FIND-TASK-001-7`.**
