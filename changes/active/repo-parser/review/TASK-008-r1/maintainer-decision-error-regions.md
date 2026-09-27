# Maintainer decision: declarations inside error regions

Decided 2026-09-27. FIND-008-1 is invalid. A tree-sitter `ERROR` node marks text the grammar could not fit, not invalid code; it often contains real declarations. The index maps names to files for candidate lookup, so dropping real names loses recall while a spurious name only adds a low-scoring candidate. Spec revision 14 REQ-002 now says extraction keeps declarations recovered inside error regions. No code change; `TASK-008-R1-error-region-provenance.md` is withdrawn.
