# Symbolic-context contract

Pass one parses caller-provided blobs one file at a time with the complete ported extractor. The index stores exact simple and qualified names, repository-relative paths, symbol spans, and IDs. Replacing/removing a file updates all of its postings without leaking stale hits. The index does not retain source text and cannot assert coverage for unseen repository files.

Pass two receives a full changed head blob and changed-line ranges. Declarations whose spans overlap changed lines and raw references on those lines become query names. Match exact `reference_name` and extractor-provided qualified candidates to index postings. All plausible hits remain visible. A hit is a candidate for context, not a proven cross-file semantic resolution. Deletion-only changes are explicitly unsupported in this initial version.

Each distinct matching name contributes one point to a candidate file. Repetition of a name does not add points. Exclude the changed file from secondary candidates; sort by descending score and then canonical path. Changed hunks take context budget first, followed by ranked relevant symbol spans. The caller supplies candidate blob text and its model tokenizer through `Fn(&str) -> usize`; the library enforces file and token limits, reports omissions, and never silently truncates a source span. An unfittable changed hunk is a typed error.
