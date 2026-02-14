//! Error types and handling patterns for vsedit.
//!
//! Provides [`VsError`], a unified error enum modeled after VS Code's
//! `vs/base/common/errors.ts`, along with a pluggable [`ErrorHandler`] trait
//! for global unexpected-error handling.

use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Core error type
// ---------------------------------------------------------------------------

/// Unified error type used throughout vsedit.
#[derive(Debug, thiserror::Error)]
pub enum VsError {
    /// The operation was cancelled.
    #[error("Cancelled")]
    Cancelled,

    /// The requested feature is not supported.
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// The requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// The requested functionality is not yet implemented.
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// An illegal argument was provided.
    #[error("Illegal argument: {0}")]
    IllegalArgument(String),

    /// The system is in an illegal state for the requested operation.
    #[error("Illegal state: {0}")]
    IllegalState(String),

    /// The target resource is read-only.
    #[error("Read-only: {0}")]
    ReadOnly(String),

    /// Permission was denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// A user-facing error with a free-form message.
    #[error("{0}")]
    User(String),

    /// Wrapper around [`std::io::Error`].
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Wrapper around [`anyhow::Error`] for ad-hoc errors.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience alias used across the codebase.
pub type Result<T> = std::result::Result<T, VsError>;

// ---------------------------------------------------------------------------
// Constructors & helpers
// ---------------------------------------------------------------------------

/// Creates a [`VsError::Cancelled`] error.
pub fn cancelled() -> VsError {
    VsError::Cancelled
}

/// Returns `true` if the error represents a cancellation.
///
/// This checks both [`VsError::Cancelled`] directly and any inner
/// `anyhow::Error` that wraps a `VsError::Cancelled`.
pub fn is_cancelled(error: &VsError) -> bool {
    match error {
        VsError::Cancelled => true,
        VsError::Other(inner) => inner.downcast_ref::<VsError>().is_some_and(is_cancelled),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Global error handler
// ---------------------------------------------------------------------------

/// Trait for handling unexpected errors globally.
///
/// Modeled after VS Code's `ErrorHandler` interface: callers use
/// [`on_unexpected_error`] to report errors, and the currently installed
/// handler receives them.
pub trait ErrorHandler: Send + Sync {
    /// Called when an unexpected error occurs.
    fn on_error(&self, error: &VsError);
}

/// The default handler simply logs errors to `eprintln!`.
struct DefaultErrorHandler;

impl ErrorHandler for DefaultErrorHandler {
    fn on_error(&self, error: &VsError) {
        eprintln!("[vsedit] unexpected error: {error}");
    }
}

static HANDLER: OnceLock<Mutex<Box<dyn ErrorHandler>>> = OnceLock::new();

fn handler() -> &'static Mutex<Box<dyn ErrorHandler>> {
    HANDLER.get_or_init(|| Mutex::new(Box::new(DefaultErrorHandler)))
}

/// Installs a custom global [`ErrorHandler`].
///
/// Replaces the current handler. Only one handler is active at a time.
pub fn set_error_handler(new_handler: Box<dyn ErrorHandler>) {
    let mut guard = handler().lock().expect("error handler mutex poisoned");
    *guard = new_handler;
}

/// Reports an unexpected error to the global [`ErrorHandler`].
///
/// Cancelled errors are silently ignored, mirroring VS Code's
/// `onUnexpectedError` behaviour.
pub fn on_unexpected_error(error: &VsError) {
    if is_cancelled(error) {
        return;
    }
    let guard = handler().lock().expect("error handler mutex poisoned");
    guard.on_error(error);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

    #[test]
    fn cancelled_constructor() {
        let err = cancelled();
        assert!(is_cancelled(&err));
        assert_eq!(err.to_string(), "Cancelled");
    }

    #[test]
    fn is_cancelled_false_for_other_variants() {
        let err = VsError::NotFound("file.txt".into());
        assert!(!is_cancelled(&err));
    }

    #[test]
    fn is_cancelled_detects_wrapped_anyhow() {
        let inner = cancelled();
        let wrapped = VsError::Other(inner.into());
        assert!(is_cancelled(&wrapped));
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(VsError::NotSupported("x".into()).to_string(), "Not supported: x");
        assert_eq!(VsError::NotFound("y".into()).to_string(), "Not found: y");
        assert_eq!(VsError::NotImplemented("z".into()).to_string(), "Not implemented: z");
        assert_eq!(
            VsError::IllegalArgument("a".into()).to_string(),
            "Illegal argument: a"
        );
        assert_eq!(
            VsError::IllegalState("b".into()).to_string(),
            "Illegal state: b"
        );
        assert_eq!(VsError::ReadOnly("c".into()).to_string(), "Read-only: c");
        assert_eq!(
            VsError::PermissionDenied("d".into()).to_string(),
            "Permission denied: d"
        );
        assert_eq!(VsError::User("oops".into()).to_string(), "oops");
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let vs_err: VsError = io_err.into();
        assert!(matches!(vs_err, VsError::Io(_)));
        assert_eq!(vs_err.to_string(), "gone");
    }

    #[test]
    fn result_alias_works() {
        fn ok_fn() -> Result<u32> {
            Ok(42)
        }
        fn err_fn() -> Result<u32> {
            Err(VsError::IllegalState("bad".into()))
        }
        assert_eq!(ok_fn().unwrap(), 42);
        assert!(err_fn().is_err());
    }

    #[test]
    fn custom_error_handler() {
        let count = Arc::new(AtomicUsize::new(0));

        struct Counter(Arc<AtomicUsize>);
        impl ErrorHandler for Counter {
            fn on_error(&self, _error: &VsError) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        set_error_handler(Box::new(Counter(Arc::clone(&count))));

        on_unexpected_error(&VsError::IllegalState("boom".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Cancelled errors should be silently ignored.
        on_unexpected_error(&cancelled());
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Restore default handler so other tests are unaffected.
        set_error_handler(Box::new(DefaultErrorHandler));
    }
}
