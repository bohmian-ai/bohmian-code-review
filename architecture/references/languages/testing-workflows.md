# Testing workflows

Three tiers apply, in priority order:

1. **Public journey:** call only `bohmian-code-review`'s public facade with in-memory source. Prove extraction → symbol map → changed-line references → candidate ranking → token-bounded context, including an unfittable budget and missing candidate bytes.
2. **Component integration:** exercise a real crate boundary, such as extracted `Node`/`RawReference` facts entering the index or resolver.
3. **Unit:** prove one pure owner or edge case, including duplicates, replacement, range validation, tie ordering, and omission classification.

A lower tier never substitutes for a missing higher one. Every user-facing
change must preserve or extend the public journey, including relevant negative
flows. Execute each new behavior one scenario at a time: RED for the expected
missing behavior, GREEN for the smallest correction, REFACTOR while all prior
scenarios remain green, then run the broader owner and journey lanes.

All lanes are credential-free, network-free, Docker-free, and checkout-free. Tests may read checked-in fixtures. Extraction goldens are immutable evidence; changing expected output requires an explicit behavior decision. The scoped mise tasks are `test:extract`, `test:index`, `test:resolve`, `test:select`, and `test:symbolic-context:journey`. `test:rust` aggregates them. `check:default` runs format, lints, and type-check; `gate` adds all tests and skill mirror verification. A missing owner target fails rather than skipping. Run the narrowest sufficient lane during iteration and the full gate for broad or release changes.

When a task names a Rust test, record its exact package, target, and selector
with `mise exec -- cargo test --locked`; prove that the selector runs a test.
Run Cargo-backed checks sequentially. Record exact RED/GREEN results, broader
lane results, `git diff --check`, and tracked/untracked diff inspection.
