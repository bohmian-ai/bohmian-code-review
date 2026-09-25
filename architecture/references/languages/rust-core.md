# Rust core practices

Use concrete structs to own state, configuration, or multi-step workflows (`SymbolIndex`, candidate resolver, context selector, public engine). Put operations that use the owner's state on inherent methods. Use a free function only when its result is stateless and deterministic from arguments alone. An enum models a closed set; a trait needs multiple real implementations or an intentional boundary. Do not move parser dependencies into `core` or duplicate extraction facts in downstream crates.

Borrow paths and source bytes across parser calls when possible. The index stores facts and size metadata, not the source bytes it parsed. Keep parsing synchronous and independent of IO. A service may call the library from a bounded worker pool; the library must not create an async runtime or hide network/filesystem work.

Validate canonical `RelativePath` values and changed-line ranges at public ingress. Use typed library errors with `thiserror`; preserve ambiguity in candidate lookup. Avoid unchecked `unwrap` on input, parsing, environment, or IO. Every new or materially changed item, including private helpers and tests, has intent-focused rustdoc; fallible methods explain `# Errors`, and possible panics are documented or removed.
