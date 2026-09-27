# Maintainer decision: rejected tree entries (closes the FIND-TASK-005-4 class)

Decided 2026-09-26 after FIND-TASK-005-4 reopened twice. This replaces the FIND-4 part of `TASK-005-R3-policy-scoped-bound-and-context-cleanup.md`; its FIND-6 part stands.

Remove the rejected-tree-entry concept. `validate_tree_entry` has two outcomes:

- **Unknown mode or malformed SHA:** the provider response is broken. The tree listing call fails with a provider error; the build publishes nothing, the pointer does not move, and the caller retries.
- **Path that is not a canonical `RelativePath`:** the entry is excluded as an unsupported path and recorded in coverage like any other ineligible file. Such a path can never be eligible, so the snapshot stays complete.

Delete the tree `rejected` list and every consumer of it (rejected-entry classification in cold build, update, PR overlay and scoped context). PR-file rejection (FIND-TASK-005-5) is unchanged. All other completeness rules are unchanged: an eligible file that fails fetch or verification still makes the build incomplete.
