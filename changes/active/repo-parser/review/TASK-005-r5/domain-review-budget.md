# Domain review: PR budget and bounded work

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..866f777a1a41a03384c14c12276a5877dcac94a2`. Reviewed boundary: PR pagination and changed-head analysis, scoped admission and fallback, selected candidate blob fetch, ranking and estimated-token packing, provider byte ceilings, and cancellation by dropping the facade future.

Authority: approved spec revision 13, especially REQ-003/008–010/012–014, INV-002/004/006; TASK-005 and R1–R4 remediation; R3 rejected-entry and R4 clock/size maintainer decisions (including withdrawn FIND-TASK-005-7); `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the spec-driven reference. I inspected the complete cumulative base-to-candidate diff and surrounding `pull_request.rs`, `snapshot.rs`, `github.rs`, `eligibility.rs`, `context.rs`, and symbolic candidate packing.

The budget paths satisfy this boundary. `PrContext::run` spends scoped index capacity on changed heads before passing only the remaining capacity to `Ingest::fallback`; every changed head still gets mandatory analysis, while unindexed eligible files remain `NotFetched` in coverage. `near` supplies uncapped neighbors for eligibility classification, so excluded entries do not spend the eligible-file ceiling. The shared blob reader takes `min(max_file_bytes, provider max_blob_bytes)` before returning verified bytes; changed heads, fallback files, selected candidates, and root policy all use that reader. Snapshot scope includes `max_file_bytes`, preventing reuse of facts built under a looser ceiling. The changed-head owner reports an oversize refusal as a file-local omission; a smaller provider ceiling remains a fetch failure. Ranking excludes all listed changed paths. Candidate packing fetches only as the symbolic packer requests a blob, then applies its file and estimated-token limits. Zero allowance explicitly reports exhaustion and avoids candidate fetches. Provider calls here are sequential and remain owned by the facade future; dropping it leaves no detached fetch.

Verification: the exact focused tests `pull_request::tests::file_ceiling_omits_oversize_heads_and_snapshots` and `pull_request::tests::changed_heads_share_the_scoped_index_ceiling` each passed (1 selected, 0 failed). The source trace covers the zero-allowance and candidate packing paths. I did not rerun the full gate, a live GitHub run, a high-volume memory measurement, or a cancellation interleaving. Token counts remain estimates by the approved contract; downstream exact request counting belongs to the consumer.

Material proposed findings: none. Prior FIND-TASK-005-3, -6, and -8 remain closed in this domain. **PASS**.

```citations
[]
```
