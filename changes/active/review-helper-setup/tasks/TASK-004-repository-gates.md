---
id: CR-SETUP-T04
kind: implementation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 2
requirements: [REQ-008, REQ-009, INV-001, INV-005, INV-006, AC-001, AC-002, AC-003, AC-004, AC-005, AC-006]
depends_on: [CR-SETUP-T03]
---

# Repository authority, CI, and integrated gate

## Outcome and Value

The repository's `cr-*` lifecycle, architecture guidance, generated skill mirror, scoped mise commands, and CI prove the integrated six-crate deliverable and public journey. This is a distinct contributor/verification outcome from runtime selection. Required execution skill: `$cr-implement`.

## Current Repository Facts

Contributor authority, six canonical skills, Claude mirror, sync script, and two skill mise tasks exist but require validation against approved Wyrd-equivalent semantics. Tasks 01–03 add Rust packages, component lanes, and the external journey; aggregate/default/gate tasks and CI remain absent.

## Owners, Scope, Consumers, and Prohibited Changes

`AGENTS.md`, architecture references, `.agents/skills/cr-*`, `scripts/sync-agent-skills.sh`, `mise.toml`, and CI own this outcome; `.claude/skills` is generated only from canonical sources. Implementers, reviewers, and CI consume them. Preserve spec → structured task → scenario TDD → independent immutable task review → integrated review → completion semantics adapted to this library. No hand-edited mirror, weakened gate, missing owner target, unrelated Wyrd product rule, or publication.

## Approach

1. Validate and repair local authority, canonical skill semantics/metadata, and links.
2. Regenerate the mirror and prove the sync/drift check.
3. Complete exact REQ-009 scoped/aggregate/default/gate tasks and CI task wiring.
4. Run the integrated gate and inspect all crate, source, fixture, and governance boundaries.

## Proof Strategy

This task is documentation, configuration, generated projection, and verification of already implemented behavior; it does not require a manufactured runtime RED. Compare canonical skills to Wyrd's lifecycle contract, validate local links/metadata, confirm mirror drift makes `check:skills-sync` fail and is then restored, and confirm missing owner/test targets fail their lanes. If any executable verification script changes, add one focused failing check for its actual behavior before fixing it, then rerun the relevant gate.

## Acceptance Criteria

- All six canonical skills and Codex metadata preserve Wyrd lifecycle semantics and resolve to local authorities; generated mirror matches canonical and drift is detectable.
- Every exact REQ-009 mise task exists with the specified owner scope and aggregate composition; CI calls the same names; an absent test target fails.
- The integrated candidate passes formatting, linting, component tests, external public journey, workspace type-check, skill sync, and `gate`; donor/runtime IO exclusions and fixture integrity remain true; diff is scoped.

## Expected Write Set and Consumer Closure

Likely surfaces: `AGENTS.md`, `architecture/**`, `.agents/skills/cr-*/**`, `scripts/sync-agent-skills.sh`, generated `.claude/skills/**` via the script, `mise.toml`, and CI workflows. A lower-owner repair needs a demonstrated integration defect and its owner/journey tests; this task cannot replace task 03's external journey with a mock.

## Verification and Evidence

Validate skill metadata/links, then run `mise run check:skills-sync`, `mise run fmt:check`, `mise run lints`, `mise run test:extract`, `mise run test:index`, `mise run test:resolve`, `mise run test:select`, `mise run test:symbolic-context:journey`, `mise run test:rust`, `mise run check:default`, `mise run gate`, and `git diff --check`. Inspect tracked/untracked changes and provide an AC-001–AC-006 evidence matrix. Missing target means failure, not a skip.

## Material Stop Conditions

Return `SPEC_REVISION_REQUIRED` if governance parity requires changing approved no-clone, crate, public journey, or acceptance contracts. Report an unavailable required gate or material Wyrd semantic conflict rather than weaken or self-approve.

## Authority Links

`changes/active/review-helper-setup/spec.md`; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/README.md`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md`; `architecture/references/languages/testing-workflows.md`.

## Implementation Evidence

Candidate: `fa62d97` → `0e6bb82` plus this record. No executable verification script changed, so no RED was manufactured.

**Governance.** All six `cr-*` skills match Wyrd's `wyrd-*` lifecycle line-for-line after name normalization; the only deltas are domain substitutions (source trust/provenance/budget for tenancy/RBAC/durability, cross-crate for cross-service, Rust/grammar layers for Python/TypeScript) and repository-local authority links. Every skill has `agents/openai.yaml` metadata naming its lifecycle route. A scan of 45 Markdown files found 0 unresolved relative links; the plain-path `implementation-plan.md` reference in `cr-spec` exists. No skill, script, or authority repair was needed.

**Negative proofs** (each restored afterwards; tree clean except intended changes):

| Probe | Result |
|---|---|
| Append a line to `.claude/skills/cr-plan/SKILL.md`, run `check:skills-sync` | exit 1, `Claude skill tree drift: >fcst.... cr-plan/SKILL.md`; `skills:sync` then `check:skills-sync` exit 0 |
| Move `tests/symbolic_context_journey.rs` aside, run `test:symbolic-context:journey` | exit 101, `no test target named symbolic_context_journey` |
| Point `test:select` at a nonexistent package | exit 101, `cannot specify features for packages outside of workspace` |

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Skills and metadata preserve Wyrd semantics, links resolve, mirror matches and drift is detectable (AC-006) | `.agents/skills/cr-*`, `scripts/sync-agent-skills.sh` unchanged | normalized diff vs Wyrd; link scan; `check:skills-sync` pass and drift probe | PASS |
| Every REQ-009 lane exists with owner scope and aggregate composition; CI calls the same names; absent target fails (AC-006) | `mise.toml` `test:rust`, `check:default`, `gate`; `.github/workflows/ci.yml` runs `mise run gate` | `mise tasks ls` lists exactly the 12 REQ-009 names; missing-target probes | PASS |
| Integrated candidate passes fmt, lints, component tests, journey, type-check, skill sync, `gate`; IO exclusions and fixtures intact; diff scoped (AC-001–AC-005) | README status updated | `mise run gate` exit 0: core 25, extract 138 + 1, index 3, resolve 1, select 4, journey 3; `rg std::(fs|net|process)|Command::new|reqwest|git2` in `crates/*/src` empty; no path/Git deps; extract tests/fixtures unchanged since `be64610`; `git diff --check` clean | PASS |

**Decisions.** The workspace type-check runs inside `check:default` rather than as a separate `check` task, keeping the lane set exactly REQ-009's. The `test:extract` commands gained `--all-features` to match every other lane. CI is one Linux job running `gate`; a multi-OS matrix was not specified.

**Scope.** Changed: `mise.toml`, `.github/workflows/ci.yml`, `README.md`, this record. `IMPLEMENTATION_PLAN.md` (maintainer edit) and the untracked TASK-003 review packet are left untouched. Non-goals (runtime behavior, publication, gate weakening, hand-edited mirror) remain excluded.
