# Citation-checked Wave 1 claims

Subject: base `60cc56e`, candidate `a10c874ff346d063714be75b46b6ec2c34b990b5`; approved spec revision 11; original TASK-003 and TASK-003-R1; prior FIND ledger in r1 verdict. Scope: complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the base-to-candidate diff. Citation verifier exited 0; all six entries were OK.

| Source ID | Reviewer | Claim | Root cause and defect class | Siblings | Citations |
|---|---|---|---|---|---|
| DOM-TRUST-1 | source-trust domain | Cancellation or deadline during an archive entry read may not interrupt promptly. | `Ingest::stream_entries` checks `Execution` per entry while the underlying archive reader carries only a byte bound; missing execution check in source reads. Proposed `CLASS_INCOMPLETE` of prior `FIND-TASK-003-2`. | `tar.entries()` iterator advance, `entry.read_to_end`, `consume_archive` reader. | `snapshot.rs:673,698,699,726,1758` (verbatim citations in domain report). |
| STD-R2-1 | repository standards | New deadline test uses a wall-clock sleep contrary to a written test rule. | The test waits for real-time deadline expiry in its transport hook; nondeterministic test trigger. | No other newly added sleep reported. | `snapshot.rs:1797` (verbatim citation in standards report). |

Task-rev A and B and publication domain proposed no findings.
