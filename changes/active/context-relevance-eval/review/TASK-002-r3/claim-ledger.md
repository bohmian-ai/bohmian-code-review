# Citation-checked Wave 1 claims

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` to `012aa47dbf2b7885d1e9ba25eb1b22c861113629`.

| Source ID | Reviewer | One-sentence claim | Root cause / defect class | Siblings | Citations |
|---|---|---|---|---|---|
| SOURCE-1 | `domain-review-source.md` | A parsed patch inconsistent with its verified head can yield a successful trial with changed names omitted. | `Change::new` converts all facade analysis errors into omissions; incorrect failure classification. | Every parsed changed Python/Rust head whose facade analysis fails; A head filter, B–D query generation, and successful report output. | `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:596`, `:835` |

`TASK-A-1` is excluded because all its citations were rejected by the mandatory verifier. The other Wave 1 reports proposed no findings.
