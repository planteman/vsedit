//! Error types and handling patterns for vsedit.
//!
//! Provides [`VsError`], a unified error enum modeled after VS Code's
//! `vs/base/common/errors.ts`, along with a pluggable [`ErrorHandler`] trait
//! for global unexpected-error handling.

use std::fmt;
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

/// Accumulated statistics for errors operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ErrorsStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ErrorsStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ErrorsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ErrorsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ErrorsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for errors.
#[derive(Debug, Clone)]
pub struct ErrorsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ErrorsValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> std::result::Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> std::result::Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ErrorsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ErrorChain – wrapping nested errors with context
// ---------------------------------------------------------------------------

/// A chain of errors with associated context messages, useful for tracing
/// the path through which an error propagated.
#[derive(Debug)]
pub struct ErrorChain {
    errors: Vec<(String, VsError)>,
}

impl ErrorChain {
    /// Creates a new chain with the given root error.
    pub fn new(error: VsError) -> Self {
        Self {
            errors: vec![(String::new(), error)],
        }
    }

    /// Adds a contextual layer to the chain.
    pub fn with_context(mut self, ctx: impl Into<String>, error: VsError) -> Self {
        self.errors.push((ctx.into(), error));
        self
    }

    /// Returns the first (root-cause) error in the chain.
    pub fn root_cause(&self) -> Option<&VsError> {
        self.errors.first().map(|(_, e)| e)
    }

    /// Returns the number of errors in the chain.
    pub fn depth(&self) -> usize {
        self.errors.len()
    }

    /// Returns all context strings in the chain.
    pub fn contexts(&self) -> Vec<&str> {
        self.errors.iter().map(|(ctx, _)| ctx.as_str()).collect()
    }

    /// Returns the highest (most severe) severity across all errors.
    pub fn highest_severity(&self) -> ErrorSeverity {
        self.errors
            .iter()
            .map(|(_, e)| e.severity())
            .max_by_key(|s| severity_rank(*s))
            .unwrap_or(ErrorSeverity::Info)
    }
}

impl fmt::Display for ErrorChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (ctx, error)) in self.errors.iter().enumerate() {
            if i > 0 {
                write!(f, "\n  caused by: ")?;
            }
            if ctx.is_empty() {
                write!(f, "{error}")?;
            } else {
                write!(f, "{ctx}: {error}")?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// error_display_format – user-friendly terminal formatting
// ---------------------------------------------------------------------------

/// Formats a `VsError` for user-friendly terminal display.
///
/// The output includes the severity prefix (e.g. `[ERROR]`), the error code
/// in parentheses, and a `"System error: "` prefix for I/O errors.
pub fn error_display_format(error: &VsError) -> String {
    let severity_label = match error.severity() {
        ErrorSeverity::Info => "INFO",
        ErrorSeverity::Warning => "WARN",
        ErrorSeverity::Error => "ERROR",
    };

    let message = match error {
        VsError::Io(e) => format!("System error: {e}"),
        other => other.to_string(),
    };

    format!("[{severity_label}] {message} ({code})", code = error.code())
}

// ---------------------------------------------------------------------------
// ErrorFilter – filter errors by criteria
// ---------------------------------------------------------------------------

/// Filters errors and error records by configurable criteria such as
/// minimum severity and variant exclusions.
#[derive(Debug, Clone)]
pub struct ErrorFilter {
    min_severity: Option<ErrorSeverity>,
    exclude_cancelled: bool,
}

impl ErrorFilter {
    /// Creates a new filter with no restrictions.
    pub fn new() -> Self {
        Self {
            min_severity: None,
            exclude_cancelled: false,
        }
    }

    /// Sets the minimum severity an error must have to pass the filter.
    pub fn min_severity(mut self, sev: ErrorSeverity) -> Self {
        self.min_severity = Some(sev);
        self
    }

    /// Excludes `Cancelled` errors from matching.
    pub fn exclude_cancelled(mut self) -> Self {
        self.exclude_cancelled = true;
        self
    }

    /// Returns `true` if the given error passes all filter criteria.
    pub fn matches(&self, error: &VsError) -> bool {
        if self.exclude_cancelled && is_cancelled(error) {
            return false;
        }
        if let Some(min) = self.min_severity {
            if severity_rank(error.severity()) < severity_rank(min) {
                return false;
            }
        }
        true
    }

    /// Filters a slice of [`ErrorRecord`]s, returning references to those
    /// that pass all criteria.
    pub fn filter_records<'a>(&self, records: &'a [ErrorRecord]) -> Vec<&'a ErrorRecord> {
        records.iter().filter(|r| self.matches(&r.error)).collect()
    }
}

impl Default for ErrorFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ErrorClassifier – categorise errors
// ---------------------------------------------------------------------------

/// Broad category of an error, useful for routing and retry logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Caused by user input or action.
    User,
    /// Caused by the operating system or environment.
    System,
    /// An internal logic error.
    Internal,
    /// A potentially transient failure that may succeed on retry.
    Transient,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::System => write!(f, "system"),
            Self::Internal => write!(f, "internal"),
            Self::Transient => write!(f, "transient"),
        }
    }
}

/// Classifies errors and determines retry eligibility.
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// Returns the [`ErrorCategory`] for the given error.
    pub fn classify(error: &VsError) -> ErrorCategory {
        match error {
            VsError::User(_)
            | VsError::IllegalArgument(_)
            | VsError::NotFound(_)
            | VsError::ReadOnly(_)
            | VsError::PermissionDenied(_) => ErrorCategory::User,

            VsError::Io(_) => ErrorCategory::System,

            VsError::Cancelled => ErrorCategory::Transient,

            VsError::NotImplemented(_)
            | VsError::NotSupported(_)
            | VsError::IllegalState(_)
            | VsError::Other(_) => ErrorCategory::Internal,
        }
    }

    /// Returns `true` if the error is potentially transient and the
    /// operation could succeed on retry.
    pub fn is_retriable(error: &VsError) -> bool {
        matches!(
            Self::classify(error),
            ErrorCategory::System | ErrorCategory::Transient
        )
    }
}

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

    #[test]
    fn errors_stats_new_defaults() {
        let stats = ErrorsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn errors_stats_record_success() {
        let mut stats = ErrorsStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn errors_stats_record_failure() {
        let mut stats = ErrorsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn errors_stats_reset() {
        let mut stats = ErrorsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn errors_stats_merge() {
        let mut a = ErrorsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ErrorsStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn errors_stats_display() {
        let mut stats = ErrorsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn errors_stats_default() {
        let stats = ErrorsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn errors_validator_accepts_valid_name() {
        let v = ErrorsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn errors_validator_rejects_empty() {
        let v = ErrorsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn errors_validator_rejects_too_long() {
        let v = ErrorsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn errors_validator_forbidden_prefix() {
        let v = ErrorsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn errors_validator_allowed_chars() {
        let v = ErrorsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn errors_validator_range() {
        let v = ErrorsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn errors_sanitize_removes_control() {
        let result = ErrorsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn errors_truncate_short_string() {
        assert_eq!(ErrorsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn errors_truncate_long_string() {
        let result = ErrorsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn errors_is_ascii_printable() {
        assert!(ErrorsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ErrorsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // ErrorChain tests
    // -----------------------------------------------------------------------

    #[test]
    fn error_chain_creation_and_depth() {
        let chain = ErrorChain::new(VsError::Cancelled);
        assert_eq!(chain.depth(), 1);
    }

    #[test]
    fn error_chain_with_context_adds_layers() {
        let chain = ErrorChain::new(VsError::Cancelled)
            .with_context("opening file", VsError::NotFound("foo.txt".into()))
            .with_context("loading project", VsError::IllegalState("bad state".into()));
        assert_eq!(chain.depth(), 3);
        let ctxs = chain.contexts();
        assert_eq!(ctxs[1], "opening file");
        assert_eq!(ctxs[2], "loading project");
    }

    #[test]
    fn error_chain_root_cause_returns_first() {
        let chain = ErrorChain::new(VsError::Cancelled)
            .with_context("wrap", VsError::NotFound("x".into()));
        let root = chain.root_cause().unwrap();
        assert!(matches!(root, VsError::Cancelled));
    }

    #[test]
    fn error_chain_highest_severity_picks_most_severe() {
        let chain = ErrorChain::new(VsError::Cancelled) // Info
            .with_context("layer", VsError::NotSupported("a".into())) // Warning
            .with_context("layer2", VsError::NotFound("b".into())); // Error
        assert_eq!(chain.highest_severity(), ErrorSeverity::Error);
    }

    #[test]
    fn error_chain_display_format() {
        let chain = ErrorChain::new(VsError::Cancelled)
            .with_context("opening file", VsError::NotFound("foo".into()));
        let display = chain.to_string();
        assert!(display.contains("Cancelled"));
        assert!(display.contains("opening file"));
    }

    // -----------------------------------------------------------------------
    // error_display_format tests
    // -----------------------------------------------------------------------

    #[test]
    fn display_format_regular_error() {
        let err = VsError::NotFound("config.json".into());
        let formatted = error_display_format(&err);
        assert!(formatted.starts_with("[ERROR]"));
        assert!(formatted.contains("NOT_FOUND"));
        assert!(formatted.contains("config.json"));
    }

    #[test]
    fn display_format_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err = VsError::Io(io_err);
        let formatted = error_display_format(&err);
        assert!(formatted.starts_with("[ERROR]"));
        assert!(formatted.contains("System error:"));
        assert!(formatted.contains("IO_ERROR"));
    }

    #[test]
    fn display_format_warning() {
        let err = VsError::NotSupported("feature X".into());
        let formatted = error_display_format(&err);
        assert!(formatted.starts_with("[WARN]"));
    }

    // -----------------------------------------------------------------------
    // ErrorFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn error_filter_matches_by_severity() {
        let filter = ErrorFilter::new().min_severity(ErrorSeverity::Warning);
        assert!(filter.matches(&VsError::NotFound("x".into()))); // Error >= Warning
        assert!(filter.matches(&VsError::NotSupported("y".into()))); // Warning >= Warning
        assert!(!filter.matches(&VsError::Cancelled)); // Info < Warning
    }

    #[test]
    fn error_filter_excludes_cancelled() {
        let filter = ErrorFilter::new().exclude_cancelled();
        assert!(!filter.matches(&VsError::Cancelled));
        assert!(filter.matches(&VsError::NotFound("x".into())));
    }

    #[test]
    fn error_filter_records() {
        let records = vec![
            ErrorRecord::new(VsError::Cancelled),
            ErrorRecord::new(VsError::NotFound("a".into())),
            ErrorRecord::new(VsError::NotSupported("b".into())),
        ];
        let filter = ErrorFilter::new().min_severity(ErrorSeverity::Warning);
        let filtered = filter.filter_records(&records);
        assert_eq!(filtered.len(), 2);
    }

    // -----------------------------------------------------------------------
    // ErrorClassifier tests
    // -----------------------------------------------------------------------

    #[test]
    fn error_classifier_categories() {
        assert_eq!(
            ErrorClassifier::classify(&VsError::User("oops".into())),
            ErrorCategory::User
        );
        assert_eq!(
            ErrorClassifier::classify(&VsError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "disk"
            ))),
            ErrorCategory::System
        );
        assert_eq!(
            ErrorClassifier::classify(&VsError::Cancelled),
            ErrorCategory::Transient
        );
        assert_eq!(
            ErrorClassifier::classify(&VsError::IllegalState("bad".into())),
            ErrorCategory::Internal
        );
    }

    #[test]
    fn error_classifier_retriable() {
        assert!(ErrorClassifier::is_retriable(&VsError::Cancelled));
        assert!(ErrorClassifier::is_retriable(&VsError::Io(
            std::io::Error::new(std::io::ErrorKind::Other, "timeout")
        )));
        assert!(!ErrorClassifier::is_retriable(&VsError::NotFound(
            "x".into()
        )));
    }
}
