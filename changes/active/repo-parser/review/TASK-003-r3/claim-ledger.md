# Citation-checked Wave 1 claims

Subject: `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2` → `d95346349988fed1d79dcb6e8b532b185e05b563`; spec revision 11. The complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs` are in scope because TASK-003 began in `741c049` before the base. All citations below passed `citations.txt`.

| Source ID / reviewer | One-sentence claim | Root cause and defect class | Citation locations | Siblings |
|---|---|---|---|---|
| `TASK-B-1` / task-rev B | Cancellation during injected storage IO can still advance the pointer or return successful reuse. | `ColdBuild::run` checks `Execution` before serialization/publication IO; `publish` has no execution handle, so interruption is unobserved at completion. | `snapshot.rs:525,552,583,602,466` | Create, read-back, persistence record, pointer CAS, reused-pointer read. |
| `DOM-TRUST-R3-1` / domain-rev source trust | Cancellation during snapshot read-back can still return `Published` and move the pointer. | Execution guard ends before the publication IO boundary; same class as `TASK-B-1`. | `snapshot.rs:525,587,602,466` | Record/create/read-back, pointer CAS, reused-pointer read. |
| `DOM-TRUST-R3-2` / domain-rev source trust | Reset `!linguist-vendored` or `!linguist-generated` incorrectly admits paths excluded by built-in rules. | `parse_attribute` collapses Git's unspecified reset state into `false`; later exclusion treats it as an explicit override. | `eligibility.rs:139,143,279,282` | Both vendored and generated/minified attribute resets. |

Paths above are under `crates/bohmian-code-review-pipeline/src/`. Full verbatim snippets and literal symbol metadata remain in the Wave 1 reports. No other Wave 1 reviewer proposed a finding.
