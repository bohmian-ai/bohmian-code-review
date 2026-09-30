//! [`ParsePool`] — the bounded CPU parsing owner.
//!
//! Parsing is the only CPU-heavy stage of indexing, and it must never run
//! inside a database transaction: the frozen upstream pipeline dispatches
//! each file into a bounded parse window on worker threads and commits the
//! results on the owning thread in **file order**, never completion order,
//! because resolution disambiguates same-named candidates by insertion
//! order. This owner reproduces exactly that contract for Rust: a caller
//! hands it one batch of already-read sources, it parses them across a
//! bounded worker set, and it returns the outcomes indexed by the batch
//! position they arrived in, so the caller's persistence loop stays
//! deterministic regardless of parse timing.
//!
//! A single file can never take the batch down. A walker error becomes that
//! file's [`Err`] outcome, and a walker panic is caught and converted to
//! [`ExtractionError::ExtractionFailed`] so unrelated files in the same batch
//! still parse and still persist — the frozen `recordParseFailure` behavior.

use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use bohmian_code_review_core::Language;
use std::num::NonZeroUsize;
use std::panic::AssertUnwindSafe;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// One already-read source file queued for bounded parsing.
///
/// The language is supplied by the caller, not re-derived here: discovery
/// owns language selection (including the project's custom extension
/// overrides) and that decision must reach the walker unchanged.
#[derive(Debug, Clone, Copy)]
pub struct ParseRequest<'a> {
    /// Repository-relative path whose bytes form the file's node identities.
    pub file_path: &'a str,
    /// The file's verified UTF-8 source text.
    pub source: &'a str,
    /// The language discovery resolved for this file.
    pub language: Language,
}

/// The per-file result of one bounded parse batch.
///
/// `None` marks a request the pool never attempted because cancellation was
/// observed first; the caller persists nothing for it and abandons the batch.
pub type ParseOutcome = Option<Result<ExtractionResult, ExtractionError>>;

/// The bounded CPU parsing owner.
///
/// Holds the [`Extractor`] that carries the parser resources and the worker
/// bound that keeps a large batch from oversubscribing the machine. It owns
/// no IO and no database handle by construction, which is what keeps CPU
/// parsing outside every database transaction.
#[derive(Debug, Clone)]
pub struct ParsePool {
    /// The extraction owner every worker dispatches through.
    extractor: Extractor,
    /// Upper bound on worker threads used for one batch.
    workers: NonZeroUsize,
}

impl ParsePool {
    /// Builds a pool over an extractor with an explicit worker bound.
    #[must_use]
    pub fn new(extractor: Extractor, workers: NonZeroUsize) -> Self {
        ParsePool { extractor, workers }
    }

    /// Builds a pool bounded by the machine's available parallelism, falling
    /// back to a single worker when the platform cannot report it.
    #[must_use]
    pub fn with_available_parallelism(extractor: Extractor) -> Self {
        let workers = std::thread::available_parallelism()
            .unwrap_or(NonZeroUsize::MIN)
            .min(NonZeroUsize::new(16).unwrap_or(NonZeroUsize::MIN));
        ParsePool::new(extractor, workers)
    }

    /// The extraction owner this pool dispatches through.
    #[must_use]
    pub fn extractor(&self) -> &Extractor {
        &self.extractor
    }

    /// The configured worker bound.
    #[must_use]
    pub fn workers(&self) -> NonZeroUsize {
        self.workers
    }

    /// Parses one batch of already-read sources and returns the outcomes in
    /// the order the requests were supplied.
    ///
    /// Workers pull from a shared cursor, so a slow file does not stall the
    /// rest of the batch, and each outcome is written into its own request
    /// slot, so completion order never leaks into the returned order. Every
    /// worker re-checks `cancelled` before claiming the next request; the
    /// requests already claimed finish, and every unclaimed request comes
    /// back as `None`.
    ///
    /// # Panics
    ///
    /// Panics only if the results mutex is poisoned, which requires a panic
    /// inside the lock — the parse itself runs outside it and its panics are
    /// caught.
    #[must_use]
    pub fn parse_batch(
        &self,
        requests: &[ParseRequest<'_>],
        cancelled: &AtomicBool,
    ) -> Vec<ParseOutcome> {
        let total = requests.len();
        if total == 0 {
            return Vec::new();
        }
        let results: Mutex<Vec<ParseOutcome>> = Mutex::new(vec![None; total]);
        let cursor = AtomicUsize::new(0);
        let workers = self.workers.get().min(total);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(request) = requests.get(index) else {
                            break;
                        };
                        let outcome = self.parse_one(request);
                        let mut slot = results.lock().expect("parse batch results mutex");
                        slot[index] = Some(outcome);
                    }
                });
            }
        });
        results.into_inner().expect("parse batch results mutex")
    }

    /// Parses one request, converting a walker panic into the same typed
    /// per-file failure a walker error produces.
    ///
    /// # Errors
    ///
    /// Returns the routed extractor's typed error, or
    /// [`ExtractionError::ExtractionFailed`] carrying the panic payload when
    /// a walker unwinds.
    fn parse_one(&self, request: &ParseRequest<'_>) -> Result<ExtractionResult, ExtractionError> {
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            self.extractor.extract_from_source_with_language(
                request.file_path,
                request.source,
                request.language,
            )
        }))
        .unwrap_or_else(|payload| {
            Err(ExtractionError::ExtractionFailed {
                language: request.language,
                message: panic_message(&payload),
            })
        })
    }
}

/// Renders a caught panic payload as the message of a per-file parse failure.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "parser panicked".to_owned()
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::config::ExtractionConfig;

    /// Builds a pool over every native language with an explicit worker bound.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn pool(workers: usize) -> ParsePool {
        ParsePool::new(
            Extractor::new(ExtractionConfig::all_native()).expect("extractor"),
            NonZeroUsize::new(workers).expect("workers"),
        )
    }

    #[test]
    /// A malformed file's failure stays local: unrelated files in the same
    /// batch still parse, and every outcome keeps its request position.
    ///
    /// # Panics
    ///
    /// Panics if the batch does not preserve source order or a clean file fails.
    fn one_failing_file_does_not_abort_the_batch() {
        let requests = [
            ParseRequest {
                file_path: "a.py",
                source: "def a():\n    return 1\n",
                language: Language::Python,
            },
            ParseRequest {
                file_path: "b.xyz",
                source: "not a language this kernel can parse\n",
                language: Language::Unknown,
            },
            ParseRequest {
                file_path: "c.py",
                source: "def c():\n    return 3\n",
                language: Language::Python,
            },
        ];
        let outcomes = pool(4).parse_batch(&requests, &AtomicBool::new(false));
        assert_eq!(outcomes.len(), 3);
        let names = |index: usize| -> Vec<String> {
            outcomes[index]
                .as_ref()
                .expect("attempted")
                .as_ref()
                .expect("parsed")
                .nodes
                .iter()
                .map(|node| node.name.clone())
                .collect()
        };
        assert!(names(0).iter().any(|name| name == "a"));
        assert!(
            outcomes[1].as_ref().expect("attempted").is_err(),
            "an unroutable file must fail alone"
        );
        assert!(names(2).iter().any(|name| name == "c"));
    }

    #[test]
    /// A syntactically malformed file extracts from its recovered tree
    /// without disturbing the valid files on either side of it.
    ///
    /// # Panics
    ///
    /// Panics if any file fails or a valid file loses its declaration.
    fn malformed_file_recovers_alone_and_later_valid_file_parses() {
        let requests = [
            ParseRequest {
                file_path: "src/a.ts",
                source: "export function lookupUser(id: string) { return id; }\n",
                language: Language::Typescript,
            },
            ParseRequest {
                file_path: "src/broken.ts",
                source: "export function broken(( {\n",
                language: Language::Typescript,
            },
            ParseRequest {
                file_path: "src/c.ts",
                source: "export function saveUser(id: string) { return id; }\n",
                language: Language::Typescript,
            },
        ];
        let outcomes = pool(2).parse_batch(&requests, &AtomicBool::new(false));
        let function_names = |index: usize| -> Vec<String> {
            outcomes[index]
                .as_ref()
                .expect("attempted")
                .as_ref()
                .expect("parsed")
                .nodes
                .iter()
                .filter(|node| node.kind == bohmian_code_review_core::NodeKind::Function)
                .map(|node| node.name.clone())
                .collect()
        };
        assert_eq!(function_names(0), vec!["lookupUser"]);
        function_names(1);
        assert_eq!(function_names(2), vec!["saveUser"]);
    }

    #[test]
    /// Every worker bound produces byte-identical, source-ordered results.
    ///
    /// # Panics
    ///
    /// Panics if a bounded batch differs from the single-worker control.
    fn bounded_workers_do_not_change_batch_results() {
        let sources: Vec<String> = (0..24)
            .map(|index| format!("def f{index}():\n    return {index}\n"))
            .collect();
        let paths: Vec<String> = (0..24).map(|index| format!("f{index}.py")).collect();
        let requests: Vec<ParseRequest<'_>> = paths
            .iter()
            .zip(sources.iter())
            .map(|(path, source)| ParseRequest {
                file_path: path.as_str(),
                source: source.as_str(),
                language: Language::Python,
            })
            .collect();
        let control = pool(1).parse_batch(&requests, &AtomicBool::new(false));
        for workers in [2, 4, 8] {
            let bounded = pool(workers).parse_batch(&requests, &AtomicBool::new(false));
            assert_eq!(bounded, control, "worker bound {workers} changed the batch");
        }
    }

    #[test]
    /// Cancellation stops claiming new work and reports the unattempted tail.
    ///
    /// # Panics
    ///
    /// Panics if a cancelled batch attempts any request.
    fn cancellation_leaves_unattempted_requests_unparsed() {
        let requests = [ParseRequest {
            file_path: "a.py",
            source: "def a():\n    return 1\n",
            language: Language::Python,
        }];
        let outcomes = pool(2).parse_batch(&requests, &AtomicBool::new(true));
        assert_eq!(outcomes, vec![None]);
    }
}
