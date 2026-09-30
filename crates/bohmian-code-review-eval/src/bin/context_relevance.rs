//! `mise run live:context-relevance`: print one source-free JSON report per
//! fixed repository, or only the failure on stderr with exit 1. Reads
//! GitHub with `GITHUB_TOKEN`, read-only.

use bohmian_code_review::{
    CoChange, DEFAULT_RANKER, Language, Ranker, RaritySymbol, SymbolOverlap,
};
use bohmian_code_review_eval::live::{LiveError, Repository, print};
use bohmian_code_review_eval::report::Arms;

/// Repositories evaluated, with their fixed cutoffs and source languages.
const REPOSITORIES: [Repository; 3] = [
    Repository {
        name: "pydantic/pydantic-ai",
        cutoff: "2026-08-23T19:15:42Z",
        languages: &[Language::Python],
        suffixes: None,
    },
    Repository {
        name: "tokio-rs/tokio",
        cutoff: "2026-05-21T13:41:55Z",
        languages: &[Language::Rust],
        suffixes: None,
    },
    Repository {
        name: "colinhacks/zod",
        cutoff: "2026-09-01T00:00:00Z",
        languages: &[Language::Typescript, Language::Tsx],
        suffixes: Some(&[".ts", ".tsx"]),
    },
];

/// Every arm by report id: S symbol overlap, C rarity-weighted symbols (the
/// baseline), E co-change alone, and F the production default fusing C and
/// E by reciprocal rank, the arm the decision is about.
const ARMS: [(&str, &dyn Ranker); 4] = [
    ("S", &SymbolOverlap),
    ("C", &RaritySymbol),
    ("E", &CoChange),
    ("F", &DEFAULT_RANKER),
];

/// Entry point: prints the reports, or the failure on stderr with exit 1.
#[tokio::main]
async fn main() {
    let result = match Arms::new(&ARMS, "C", "F") {
        Ok(arms) => print(&REPOSITORIES, arms).await,
        Err(error) => Err(LiveError::from(error)),
    };
    if let Err(error) = result {
        eprintln!("context_relevance failed: {error}");
        std::process::exit(1);
    }
}

/// The fixed experiment configuration.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review_eval::sample::Discovery;

    /// Each fixed repository has a valid cutoff and a nonempty language
    /// set, the three samples are the approved ones, and the S/C/E/F arms
    /// pair against C and decide F.
    #[test]
    fn repositories_and_arms_are_the_fixed_experiment() {
        let fixed: Vec<_> = REPOSITORIES
            .iter()
            .map(|repo| {
                assert!(Discovery::new("main", repo.cutoff).is_ok(), "{}", repo.name);
                assert!(!repo.languages.is_empty(), "{}", repo.name);
                (repo.name, repo.cutoff)
            })
            .collect();
        assert_eq!(
            fixed,
            [
                ("pydantic/pydantic-ai", "2026-08-23T19:15:42Z"),
                ("tokio-rs/tokio", "2026-05-21T13:41:55Z"),
                ("colinhacks/zod", "2026-09-01T00:00:00Z"),
            ]
        );
        assert_eq!(ARMS.map(|(id, _)| id), ["S", "C", "E", "F"]);
        let arms = Arms::new(&ARMS, "C", "F").expect("fixed arms are valid");
        assert_eq!((arms.baseline(), arms.decision()), ("C", "F"));
    }
}
