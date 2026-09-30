# Reference router

References explain how to apply `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md`; they do not create a second architecture. Load only the slice needed by a change.

| Reference | Load when changing |
|---|---|
| [spec-driven development](languages/spec-driven-development.md) | Specifications, tasks, acceptance review, or completion records |
| [implementation execution](languages/implementation-execution.md) | Bounded task implementation and verification evidence |
| [Rust core](languages/rust-core.md) | Crate APIs, ownership, parser execution, errors, and rustdoc |
| [testing workflows](languages/testing-workflows.md) | Component lanes, the public journey, CI, and gates |
| [symbolic context](domain/symbolic-context.md) | Extraction facts, index lookup, candidate resolution, ranking, and token packing |
| [pipeline consumer ergonomics](domain/pipeline-consumer-ergonomics.md) | The pipeline crate's public API, its injected store traits, or how a consumer such as Wyrd calls it |

The first four references adapt the relevant Wyrd repository guidance to this Rust-only library. The symbolic-context reference is repository-specific. Wyrd SQL, audit, server, PyO3, UI, and product doctrines do not apply here.
