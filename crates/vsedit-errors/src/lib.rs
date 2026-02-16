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
// Error classification & severity
// ---------------------------------------------------------------------------

/// Severity level for an error, useful for logging and UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// Informational – the operation succeeded but with caveats.
    Info,
    /// A warning that does not prevent the operation from completing.
    Warning,
    /// A hard error that prevented the operation.
    Error,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl VsError {
    /// Returns the [`ErrorSeverity`] that best matches this error variant.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            VsError::Cancelled => ErrorSeverity::Info,
            VsError::NotImplemented(_) | VsError::NotSupported(_) => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }

    /// Returns `true` if this is a user-facing error (as opposed to an
    /// internal/system error).
    pub fn is_user_facing(&self) -> bool {
        matches!(
            self,
            VsError::User(_)
                | VsError::NotFound(_)
                | VsError::ReadOnly(_)
                | VsError::PermissionDenied(_)
        )
    }

    /// Returns the inner message string for variants that carry one,
    /// or `None` for structural variants like `Cancelled`, `Io`, and `Other`.
    pub fn message(&self) -> Option<&str> {
        match self {
            VsError::NotSupported(m)
            | VsError::NotFound(m)
            | VsError::NotImplemented(m)
            | VsError::IllegalArgument(m)
            | VsError::IllegalState(m)
            | VsError::ReadOnly(m)
            | VsError::PermissionDenied(m)
            | VsError::User(m) => Some(m.as_str()),
            VsError::Cancelled | VsError::Io(_) | VsError::Other(_) => None,
        }
    }

    /// Returns an error code string suitable for serialisation or logging.
    pub fn code(&self) -> &'static str {
        match self {
            VsError::Cancelled => "CANCELLED",
            VsError::NotSupported(_) => "NOT_SUPPORTED",
            VsError::NotFound(_) => "NOT_FOUND",
            VsError::NotImplemented(_) => "NOT_IMPLEMENTED",
            VsError::IllegalArgument(_) => "ILLEGAL_ARGUMENT",
            VsError::IllegalState(_) => "ILLEGAL_STATE",
            VsError::ReadOnly(_) => "READ_ONLY",
            VsError::PermissionDenied(_) => "PERMISSION_DENIED",
            VsError::User(_) => "USER_ERROR",
            VsError::Io(_) => "IO_ERROR",
            VsError::Other(_) => "OTHER",
        }
    }
}

// ---------------------------------------------------------------------------
// ErrorRecord – structured error with context
// ---------------------------------------------------------------------------

/// A structured error record that pairs a [`VsError`] with contextual
/// metadata such as a source location and timestamp.
#[derive(Debug)]
pub struct ErrorRecord {
    /// The underlying error.
    pub error: VsError,
    /// Human-readable description of where the error originated.
    pub source_location: Option<String>,
    /// Monotonic timestamp (seconds since an arbitrary epoch) when the error
    /// was recorded.
    pub timestamp: f64,
    /// Severity override – if `None`, derived from the error variant.
    severity_override: Option<ErrorSeverity>,
}

impl ErrorRecord {
    /// Creates a new `ErrorRecord` with the given error and current timestamp.
    pub fn new(error: VsError) -> Self {
        Self {
            error,
            source_location: None,
            timestamp: 0.0,
            severity_override: None,
        }
    }

    /// Builder method: attach a source location string.
    pub fn with_source_location(mut self, loc: impl Into<String>) -> Self {
        self.source_location = Some(loc.into());
        self
    }

    /// Builder method: attach a timestamp.
    pub fn with_timestamp(mut self, ts: f64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Builder method: override the default severity.
    pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity_override = Some(severity);
        self
    }

    /// Returns the effective severity (override or derived from the error).
    pub fn severity(&self) -> ErrorSeverity {
        self.severity_override.unwrap_or_else(|| self.error.severity())
    }

    /// Returns `true` if this record carries a source location.
    pub fn has_source_location(&self) -> bool {
        self.source_location.is_some()
    }
}

impl std::fmt::Display for ErrorRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.severity(), self.error)?;
        if let Some(loc) = &self.source_location {
            write!(f, " (at {loc})")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ErrorAccumulator – collect multiple errors
// ---------------------------------------------------------------------------

/// Collects multiple errors so that an operation can report all failures at
/// once rather than stopping at the first one.
#[derive(Debug, Default)]
pub struct ErrorAccumulator {
    records: Vec<ErrorRecord>,
}

impl ErrorAccumulator {
    /// Creates an empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a raw [`VsError`] into the accumulator.
    pub fn push(&mut self, error: VsError) {
        self.records.push(ErrorRecord::new(error));
    }

    /// Pushes a fully-formed [`ErrorRecord`].
    pub fn push_record(&mut self, record: ErrorRecord) {
        self.records.push(record);
    }

    /// Returns the number of accumulated errors.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if no errors have been accumulated.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns a slice of all accumulated records.
    pub fn records(&self) -> &[ErrorRecord] {
        &self.records
    }

    /// Consumes the accumulator and returns the collected records.
    pub fn into_records(self) -> Vec<ErrorRecord> {
        self.records
    }

    /// Returns the number of errors at or above the given severity.
    pub fn count_at_severity(&self, min_severity: ErrorSeverity) -> usize {
        let min = severity_rank(min_severity);
        self.records
            .iter()
            .filter(|r| severity_rank(r.severity()) >= min)
            .count()
    }

    /// Returns `true` if any accumulated error is at [`ErrorSeverity::Error`].
    pub fn has_hard_errors(&self) -> bool {
        self.count_at_severity(ErrorSeverity::Error) > 0
    }

    /// Converts into a single [`VsError`] summarising all collected errors.
    /// Returns `None` if the accumulator is empty.
    pub fn into_combined_error(self) -> Option<VsError> {
        if self.records.is_empty() {
            return None;
        }
        if self.records.len() == 1 {
            return Some(
                self.records
                    .into_iter()
                    .next()
                    .expect("checked len")
                    .error,
            );
        }
        let messages: Vec<String> = self
            .records
            .iter()
            .map(|r| r.error.to_string())
            .collect();
        Some(VsError::IllegalState(format!(
            "multiple errors: {}",
            messages.join("; ")
        )))
    }
}

/// Maps [`ErrorSeverity`] to an integer for comparison.
fn severity_rank(s: ErrorSeverity) -> u8 {
    match s {
        ErrorSeverity::Info => 0,
        ErrorSeverity::Warning => 1,
        ErrorSeverity::Error => 2,
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validates that `value` is not empty, returning an [`IllegalArgument`]
/// error if it is.
pub fn require_non_empty(name: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        Err(VsError::IllegalArgument(format!(
            "{name} must not be empty"
        )))
    } else {
        Ok(())
    }
}

/// Validates that `value` is within `min..=max` (inclusive).
pub fn require_in_range(name: &str, value: i64, min: i64, max: i64) -> Result<()> {
    if value < min || value > max {
        Err(VsError::IllegalArgument(format!(
            "{name} must be in range {min}..={max}, got {value}"
        )))
    } else {
        Ok(())
    }
}

/// Validates that `value` is `Some`, returning [`IllegalArgument`] otherwise.
pub fn require_some<T>(name: &str, value: Option<T>) -> Result<T> {
    value.ok_or_else(|| VsError::IllegalArgument(format!("{name} is required")))
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

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn error_severity_classification() {
        assert_eq!(cancelled().severity(), ErrorSeverity::Info);
        assert_eq!(
            VsError::NotImplemented("x".into()).severity(),
            ErrorSeverity::Warning
        );
        assert_eq!(
            VsError::NotSupported("x".into()).severity(),
            ErrorSeverity::Warning
        );
        assert_eq!(
            VsError::IllegalState("x".into()).severity(),
            ErrorSeverity::Error
        );
        assert_eq!(
            VsError::NotFound("x".into()).severity(),
            ErrorSeverity::Error
        );
    }

    #[test]
    fn error_code_strings() {
        assert_eq!(cancelled().code(), "CANCELLED");
        assert_eq!(VsError::NotFound("x".into()).code(), "NOT_FOUND");
        assert_eq!(VsError::ReadOnly("x".into()).code(), "READ_ONLY");
        assert_eq!(VsError::User("x".into()).code(), "USER_ERROR");
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "e");
        assert_eq!(VsError::Io(io_err).code(), "IO_ERROR");
    }

    #[test]
    fn error_message_extraction() {
        assert_eq!(VsError::User("hello".into()).message(), Some("hello"));
        assert_eq!(
            VsError::IllegalArgument("bad".into()).message(),
            Some("bad")
        );
        assert_eq!(cancelled().message(), None);
    }

    #[test]
    fn is_user_facing_classification() {
        assert!(VsError::User("x".into()).is_user_facing());
        assert!(VsError::NotFound("x".into()).is_user_facing());
        assert!(VsError::ReadOnly("x".into()).is_user_facing());
        assert!(VsError::PermissionDenied("x".into()).is_user_facing());
        assert!(!VsError::IllegalState("x".into()).is_user_facing());
        assert!(!cancelled().is_user_facing());
    }

    #[test]
    fn error_severity_display() {
        assert_eq!(ErrorSeverity::Info.to_string(), "info");
        assert_eq!(ErrorSeverity::Warning.to_string(), "warning");
        assert_eq!(ErrorSeverity::Error.to_string(), "error");
    }

    #[test]
    fn error_record_builder() {
        let rec = ErrorRecord::new(VsError::NotFound("file.rs".into()))
            .with_source_location("editor::open")
            .with_timestamp(42.5)
            .with_severity(ErrorSeverity::Warning);

        assert_eq!(rec.severity(), ErrorSeverity::Warning);
        assert!(rec.has_source_location());
        assert_eq!(rec.timestamp, 42.5);
        assert_eq!(
            rec.to_string(),
            "[warning] Not found: file.rs (at editor::open)"
        );
    }

    #[test]
    fn error_record_default_severity() {
        let rec = ErrorRecord::new(VsError::IllegalState("bad".into()));
        assert_eq!(rec.severity(), ErrorSeverity::Error);
        assert!(!rec.has_source_location());
    }

    #[test]
    fn error_accumulator_basics() {
        let mut acc = ErrorAccumulator::new();
        assert!(acc.is_empty());
        assert_eq!(acc.len(), 0);

        acc.push(VsError::NotFound("a".into()));
        acc.push(VsError::ReadOnly("b".into()));
        assert_eq!(acc.len(), 2);
        assert!(!acc.is_empty());
        assert!(acc.has_hard_errors());
    }

    #[test]
    fn error_accumulator_combined_single() {
        let mut acc = ErrorAccumulator::new();
        acc.push(VsError::NotFound("only".into()));
        let combined = acc.into_combined_error().unwrap();
        assert_eq!(combined.to_string(), "Not found: only");
    }

    #[test]
    fn error_accumulator_combined_multiple() {
        let mut acc = ErrorAccumulator::new();
        acc.push(VsError::NotFound("a".into()));
        acc.push(VsError::ReadOnly("b".into()));
        let combined = acc.into_combined_error().unwrap();
        assert!(combined.to_string().contains("multiple errors"));
        assert!(combined.to_string().contains("Not found: a"));
        assert!(combined.to_string().contains("Read-only: b"));
    }

    #[test]
    fn error_accumulator_empty_combined() {
        let acc = ErrorAccumulator::new();
        assert!(acc.into_combined_error().is_none());
    }

    #[test]
    fn error_accumulator_severity_counts() {
        let mut acc = ErrorAccumulator::new();
        acc.push(cancelled()); // Info
        acc.push(VsError::NotImplemented("x".into())); // Warning
        acc.push(VsError::IllegalState("y".into())); // Error

        assert_eq!(acc.count_at_severity(ErrorSeverity::Info), 3);
        assert_eq!(acc.count_at_severity(ErrorSeverity::Warning), 2);
        assert_eq!(acc.count_at_severity(ErrorSeverity::Error), 1);
    }

    #[test]
    fn require_non_empty_validation() {
        assert!(require_non_empty("name", "hello").is_ok());
        let err = require_non_empty("name", "").unwrap_err();
        assert_eq!(err.code(), "ILLEGAL_ARGUMENT");
        assert!(err.to_string().contains("name must not be empty"));
    }

    #[test]
    fn require_in_range_validation() {
        assert!(require_in_range("port", 8080, 1, 65535).is_ok());
        assert!(require_in_range("port", 0, 1, 65535).is_err());
        assert!(require_in_range("port", 70000, 1, 65535).is_err());
        let err = require_in_range("port", 0, 1, 65535).unwrap_err();
        assert!(err.to_string().contains("1..=65535"));
    }

    #[test]
    fn require_some_validation() {
        assert_eq!(require_some("val", Some(42)).unwrap(), 42);
        let err = require_some::<i32>("val", None).unwrap_err();
        assert!(err.to_string().contains("val is required"));
    }
}
