# bohmian-code-review

Rust helpers for in-memory symbolic context in a no-clone code-review service. The first planned capability is a two-pass file picker: extract symbols from caller-supplied blobs, then rank context files from changed-line references under a token budget.

Implementation is pending. The [active specification](changes/active/review-helper-setup/spec.md) is a draft; the [implementation handoff](changes/active/review-helper-setup/implementation-plan.md) records the intended repository, crate, and journey setup. [Architecture](architecture/bohmian-code-review-design.md) and [contributor rules](AGENTS.md) are repository authority. Canonical agent skills live in `.agents/skills`; `.claude/skills` is generated from them.
