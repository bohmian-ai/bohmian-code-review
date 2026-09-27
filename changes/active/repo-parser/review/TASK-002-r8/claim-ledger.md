# Citation-checked Wave 1 claims

Subject: `ab2fa3a..3979b427ed52e298f5fa88ef0c00bb324f0acc6b`; TASK-002 Revision 12 delta. Earlier TASK-002 scope passed in r7. Prior `FIND-TASK-002-1` through `FIND-TASK-002-12` were closed in r7; no Wave 1 claim reopens them.

| Source IDs / reviewer | Citation | Claim | Root cause / defect class | Siblings |
|---|---|---|---|---|
| `TASK-B-1` / task-rev B; `S-1` / repo-rev | `crates/bohmian-code-review-pipeline/src/github.rs:819`, `Ok(Some(chunk)) => body.extend_from_slice(&chunk),` | A headerless 403 can make rate-limit classification copy more than its 4096-byte inspection bound before truncation. | Copy before enforcing remaining prefix capacity; bound applied after allocation. | `read_bounded` checks before copy; `Archive::chunk` checks before returning. No same-pattern sibling claimed. |

Task-rev A and both domain reviewers proposed no findings. Citation check: both entries OK after correcting their symbol metadata; the initial metadata-only REJECTs are described in the verdict.
