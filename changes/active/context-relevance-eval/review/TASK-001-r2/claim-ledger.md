# Citation-checked Wave 1 claims

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..a1342a5a3c154b7678cf59764dab1ceb0d70cc76`.

The final citation verifier accepted all four claims. The citations below identify candidate source; the Wave 2 reviewer independently checks each claim from source.

| Source ID and reviewer | One-sentence claim | Root cause and defect class | Sibling locations | Candidate citations |
|---|---|---|---|---|
| `SRC-R2-1`, source trust | Repeated canonical PR file paths can count as two source files and satisfy the other-file condition. | `Listing::push` appends entries without checking path uniqueness; incomplete provider-input validation. | `sample.rs:235` producer; `sample.rs:319`, `:334`, `:338` consumers. | `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:235` `self.files.extend(page.files);`; `:319` `screen.changed_lines.push(changed);`; `:334` `(MIN_SOURCE_FILES..=MAX_SOURCE_FILES).contains(&self.source.len())`; `:338` `&& (0..self.source.len())` |
| `SRC-R2-2`, source trust | Impossible date or clock values pass timestamp validation and can enter cutoff and PR ordering. | `is_timestamp` checks shape only; incomplete value validation. | `sample.rs:141` cutoff, `:169` updated, `:175` merged, shared validator `:207-209`. | `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:207` `fn is_timestamp(text: &str) -> bool {`; `:209` `text.len() == SHAPE.len()`; `:141` `if !is_timestamp(cutoff) {`; `:169` `if !is_timestamp(&listed.updated_at) {`; `:175` `if !is_timestamp(&merged_at) {` |
| `SAMPLING-1`, sampling | Impossible date or clock values pass timestamp validation and can enter cutoff and PR ordering. | `is_timestamp` checks shape only; incomplete value validation. | `sample.rs:141` cutoff, `:169` updated, `:175` merged, shared validator `:209`. | `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:209` `text.len() == SHAPE.len()` |
| `STD-R2-1`, standards | The evaluator defines its Git and sampling workflow as free functions despite the written concrete-owner rule. | Multi-step stateful workflow has no cohesive owner; repository-rule violation. | `main.rs:96` listing, `:117` snapshot, `:156` qualify, `:197` select, `:251` run. | `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:96` `async fn listing(`; `:117` `async fn base_snapshot(`; `:156` `async fn qualify(`; `:197` `async fn select(`; `:251` `async fn run() -> Result<(), Box<dyn Error>> {` |

Prior findings: `FIND-TASK-001-1` listing at the 3,000-file provider ceiling; `FIND-TASK-001-2` rustdoc on the test module and PATCH fixture. Review whether either class is incomplete before assigning a new ID.
