# Citation-checked Wave 1 claims

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..31e71a2e8867215e0c3f8958bdcee5f94da17ad4`. All cited locations passed `mise run review:verify-citations`.

| Source ID | Reviewer | Cited locations | Claim; root cause; defect class; sibling locations |
|---|---|---|---|
| TASK-A-1 | task A | `pull_request.rs:184,228` | Scoped PR ingest passes zero rejected head-tree entries, losing rejection counts and potentially setting overlay complete. Root cause: scoped branch drops available rejection count. Defect class: incomplete coverage propagation. Siblings: no-snapshot scoped PR coverage and persisted context. |
| TASK-A-2 | task A | `github.rs:1404`; `pull_request.rs:234,236` | A rejected PR file with a canonical changed path can be selected as secondary context. Root cause: rejection loses validated path identity and `changed` uses only accepted PR files. Defect class: changed-path exclusion gap. Siblings: ranking exclusion and scoped-neighbor seed. |
| TASK-B-1 | task B | `github.rs:975`; `snapshot.rs:139`; `pull_request.rs:184` | An invalid entry proven ineligible by its validated path still prevents snapshot publication; scoped PR drops unknown rejected entries. Root cause: tree rejection collapses classification facts to a count. Defect class: classification before completeness. Siblings: cold build, update, warm overlay, scoped PR. |
| DOMAIN-DUR-1 | durability | `snapshot.rs:593,1015,654`; `github.rs:1382`; `eligibility.rs:278`; `pull_request.rs:170` | Invalid-SHA entry with validated unsupported path remains incomplete. Root cause: rejected tree entries reduced to count before eligibility. Defect class: lost classification metadata. Siblings: cold build and PR overlay. |

Standards and source-trust reviewers proposed no findings. Prior IDs are FIND-TASK-005-1 through FIND-TASK-005-4; preserve them for any same-root-cause claim.
