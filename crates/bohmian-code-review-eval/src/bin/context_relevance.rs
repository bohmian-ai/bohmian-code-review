//! `mise run live:context-relevance`: print one source-free JSON report per
//! fixed repository, or only the failure on stderr with exit 1. Reads
//! GitHub with `GITHUB_TOKEN`, read-only.

use bohmian_code_review_eval::live::print;

/// Entry point: prints the reports, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    if let Err(error) = print().await {
        eprintln!("context_relevance failed: {error}");
        std::process::exit(1);
    }
}
