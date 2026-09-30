# Citation-checked Wave 1 claims

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..e52510f112b822f7f7692535b26ae81e53ac4b03`. All citation entries for these claims returned `OK` in `citations.txt`; the verbatim citation records are in the named Wave 1 reports. This ledger contains claims only, not their supporting reasoning or proposed verdicts.

| Source and reviewer | Claim | Root cause and defect class | Cited locations and sibling sinks |
|---|---|---|---|
| `TASK-A-1`, task A | An unchanged tree at a new commit can reuse a snapshot without incorporating newly merged PRs. | Tree equality decides reuse of commit-scoped history; incorrect reuse predicate. | `update.rs:93`, `snapshot.rs:706,712`, `pull_request.rs:223`; direct cold build, refresh, and PR context. |
| `TASK-B-1`, task B | Loaded facts may carry malformed persisted reference names yet load successfully. | `IndexedFileFacts::validate` does not validate the new references field; incomplete boundary validation. | `core/src/facts.rs:119,128`, `index/src/lib.rs:218,224`; facade load, snapshot load, and exported postings. |
| `HIST-1`, history domain | At the page cap, an incremental read can succeed after ten qualified PRs without joining the prior snapshot history. | Cold-depth proof stands in for proof of the prior boundary; incomplete incremental completion rule. | `history.rs:399,400,406,432`, `update.rs:120,124`, `snapshot.rs:723,855`, `pull_request.rs:462`; incremental and fallback publication, then PR context. Prior ID: `FIND-ranker-pipeline-2`. |
| `DOMAIN-PROVIDER-1`, provider domain | PR listing entries can omit or disagree on base repository identity while being accepted for history and evaluation. | Shared listed-PR wire/conversion drops the base repository id; missing source identity validation. | `github.rs:749,868,1476,1596,1689`, `history.rs:408`, `eval/src/live.rs:278`; closed listing and all-PR listing. |

No other Wave 1 reviewer proposed a finding. Prior `FIND-ranker-pipeline-1` and `FIND-ranker-pipeline-3` require closure review against the cumulative candidate.
