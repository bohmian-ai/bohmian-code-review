# Citation-checked Wave 1 claims

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..62346cb030354596cdf3dd7596de63dbd8d99fcb`. Authority includes the r3 maintainer decision, which replaces the FIND-4 portion of R3. `citations.txt` accepted TASK-A-1 and DOMAIN-TRUST-1. All TASK-B-1 citations were rejected for symbols that the citation checker could not locate, so TASK-B-1 is dropped as a finding; its warm-reuse scenario may be inspected independently when testing TASK-A-1's root cause.

| Source ID / reviewer | Citations | One-sentence claim | Root cause and defect class | Siblings |
|---|---|---|---|---|
| TASK-A-1 / task A | `eligibility.rs:278`; `pull_request.rs:423,474` | Optional tree size permits an actual blob over `PrContextLimits::max_file_bytes` into changed-head analysis/index and selected-candidate packing. | Fetched PR bytes are not checked against the facade's file limit after optional tree-metadata admission; INCORRECT/VIOLATION. | Changed-head analysis and scoped index admission; selected candidate fetch and packing; inspect warm reused facts for the same limit. |
| DOMAIN-TRUST-1 / source trust | `snapshot.rs:746`; `eligibility.rs:273`; `pull_request.rs:163` | A valid executable root `.gitattributes` blob is ignored, so generated/vendored policy may be absent from eligibility while coverage says complete. | Shared root policy loader accepts only `TreeEntryMode::File` although executable is an admitted blob mode; INCORRECT. | Cold snapshot publication, update pointer, warm PR overlay, scoped eligibility. |

Wave 1 reports with no claims: `standards-review.md`, `domain-review-budget.md`, `domain-review-durability.md`.
