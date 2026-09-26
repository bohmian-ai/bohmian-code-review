//! Caller-supplied deadline and cancellation for one facade invocation.
//!
//! Every provider request checks [`Execution`] before it starts and bounds
//! its own timeout by the remaining time, so a facade can never outlive the
//! deadline its caller chose or ignore a cancellation request.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Why an invocation stopped before finishing its work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Interrupted {
    /// The caller cancelled the invocation.
    #[error("invocation cancelled by caller")]
    Cancelled,
    /// The caller's deadline passed.
    #[error("invocation deadline exceeded")]
    DeadlineExceeded,
}

/// Shared cancellation flag a caller may trip from another thread.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that has not been cancelled.
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Request cancellation; observed by the next [`Execution::check`].
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Deadline plus cancellation for one facade invocation.
#[derive(Debug, Clone)]
pub struct Execution {
    deadline: Instant,
    cancel: CancelToken,
}

impl Execution {
    /// Bound an invocation by an absolute deadline and a cancellation token.
    pub fn new(deadline: Instant, cancel: CancelToken) -> Execution {
        Execution { deadline, cancel }
    }

    /// Bound an invocation by `timeout` from now with a fresh token.
    pub fn with_timeout(timeout: Duration) -> Execution {
        Execution::new(Instant::now() + timeout, CancelToken::new())
    }

    /// The cancellation token, for callers that keep a handle.
    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    /// Time left before the deadline, or `None` once it has passed.
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }

    /// Time left, failing when cancelled or past the deadline.
    ///
    /// # Errors
    ///
    /// Returns [`Interrupted::Cancelled`] when the token was tripped, then
    /// [`Interrupted::DeadlineExceeded`] when no time remains.
    pub fn check(&self) -> Result<Duration, Interrupted> {
        if self.cancel.is_cancelled() {
            return Err(Interrupted::Cancelled);
        }
        self.remaining().ok_or(Interrupted::DeadlineExceeded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancellation wins over remaining time; an elapsed deadline fails.
    #[test]
    fn check_reports_cancellation_then_deadline() {
        let execution = Execution::with_timeout(Duration::from_secs(60));
        assert!(execution.check().is_ok());
        execution.cancel_token().cancel();
        assert_eq!(execution.check(), Err(Interrupted::Cancelled));

        let expired = Execution::new(Instant::now(), CancelToken::new());
        assert_eq!(expired.check(), Err(Interrupted::DeadlineExceeded));
        assert_eq!(expired.remaining(), None);
    }
}
