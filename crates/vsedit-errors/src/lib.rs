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

// ---------------------------------------------------------------------------
// Error chain formatting
// ---------------------------------------------------------------------------

/// Format an error chain as a multi-line string with indentation.
pub fn format_error_chain(chain: &ErrorChain) -> String {
    let mut out = String::new();
    for (i, (ctx, error)) in chain.errors.iter().enumerate() {
        let indent = "  ".repeat(i);
        if ctx.is_empty() {
            out.push_str(&format!("{indent}[{}] {error}\n", error.code()));
        } else {
            out.push_str(&format!("{indent}[{}] {ctx}: {error}\n", error.code()));
        }
    }
    out
}

/// Format an error chain as a single-line summary (suitable for logs).
pub fn format_error_chain_oneline(chain: &ErrorChain) -> String {
    chain
        .errors
        .iter()
        .map(|(ctx, error)| {
            if ctx.is_empty() {
                error.to_string()
            } else {
                format!("{ctx}: {error}")
            }
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

// ---------------------------------------------------------------------------
// Error rate tracking
// ---------------------------------------------------------------------------

/// Tracks error rates over a sliding window of time buckets.
#[derive(Debug, Clone)]
pub struct ErrorRateTracker {
    /// Each bucket counts errors in a time window.
    buckets: Vec<u64>,
    /// Duration of each bucket in seconds.
    bucket_duration: u64,
    /// Index of the current bucket.
    current_bucket: usize,
    /// Timestamp when the current bucket started.
    current_bucket_start: u64,
}

impl ErrorRateTracker {
    /// Create a new tracker with the given number of buckets and bucket duration.
    pub fn new(num_buckets: usize, bucket_duration: u64) -> Self {
        Self {
            buckets: vec![0; num_buckets.max(1)],
            bucket_duration: bucket_duration.max(1),
            current_bucket: 0,
            current_bucket_start: 0,
        }
    }

    /// Record an error at the given timestamp.
    pub fn record_error(&mut self, timestamp: u64) {
        self.advance_to(timestamp);
        self.buckets[self.current_bucket] += 1;
    }

    /// Advance time, zeroing out any buckets that have been skipped.
    fn advance_to(&mut self, timestamp: u64) {
        if timestamp < self.current_bucket_start {
            return;
        }
        let elapsed = timestamp - self.current_bucket_start;
        let buckets_to_advance = (elapsed / self.bucket_duration) as usize;
        if buckets_to_advance == 0 {
            return;
        }
        let n = self.buckets.len();
        for i in 1..=buckets_to_advance.min(n) {
            let idx = (self.current_bucket + i) % n;
            self.buckets[idx] = 0;
        }
        self.current_bucket = (self.current_bucket + buckets_to_advance) % n;
        self.current_bucket_start += buckets_to_advance as u64 * self.bucket_duration;
    }

    /// Total errors across all buckets.
    pub fn total_errors(&self) -> u64 {
        self.buckets.iter().sum()
    }

    /// Average errors per bucket.
    pub fn average_rate(&self) -> f64 {
        self.total_errors() as f64 / self.buckets.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Error context enrichment
// ---------------------------------------------------------------------------

/// Enriches a `VsError` with additional context key-value pairs.
#[derive(Debug)]
pub struct EnrichedError {
    pub error: VsError,
    pub context: Vec<(String, String)>,
}

impl EnrichedError {
    /// Wrap an error with no initial context.
    pub fn new(error: VsError) -> Self {
        Self { error, context: Vec::new() }
    }

    /// Add a key-value context pair.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    /// Format the enriched error for display.
    pub fn format(&self) -> String {
        let mut out = self.error.to_string();
        if !self.context.is_empty() {
            let ctx: Vec<String> = self.context.iter().map(|(k, v)| format!("{k}={v}")).collect();
            out.push_str(&format!(" [{}]", ctx.join(", ")));
        }
        out
    }
}

impl fmt::Display for EnrichedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

// ---------------------------------------------------------------------------
// Error recovery suggestions
// ---------------------------------------------------------------------------

/// Returns a user-facing recovery suggestion for the given error.
pub fn recovery_suggestion(error: &VsError) -> &'static str {
    match error {
        VsError::Cancelled => "The operation was cancelled. You can try again.",
        VsError::NotFound(_) => "The requested resource was not found. Check the path or name.",
        VsError::PermissionDenied(_) => "Permission was denied. Check file permissions or authentication.",
        VsError::ReadOnly(_) => "The resource is read-only. Open it in a writable mode.",
        VsError::NotImplemented(_) => "This feature is not yet implemented. Check for updates.",
        VsError::NotSupported(_) => "This operation is not supported in the current environment.",
        VsError::IllegalArgument(_) => "An invalid argument was provided. Check the input values.",
        VsError::IllegalState(_) => "The system is in an unexpected state. Try restarting.",
        VsError::User(_) => "Please review the error message and correct the issue.",
        VsError::Io(_) => "A system I/O error occurred. Check disk space and connectivity.",
        VsError::Other(_) => "An unexpected error occurred. Check the logs for details.",
    }
}

/// Returns `true` if the error typically requires user intervention to resolve.
pub fn requires_user_action(error: &VsError) -> bool {
    matches!(
        error,
        VsError::User(_)
            | VsError::IllegalArgument(_)
            | VsError::NotFound(_)
            | VsError::PermissionDenied(_)
            | VsError::ReadOnly(_)
    )
}

// ---------------------------------------------------------------------------
// Error categorization and utility helpers
// ---------------------------------------------------------------------------

/// Classify an error as transient (might succeed on retry) or permanent.
pub fn is_transient(error: &VsError) -> bool {
    matches!(error, VsError::Cancelled | VsError::Io(_))
}

/// Classify an error as a permission-related issue.
pub fn is_permission_error(error: &VsError) -> bool {
    matches!(
        error,
        VsError::PermissionDenied(_) | VsError::ReadOnly(_)
    )
}

/// Extract the inner message string from a VsError variant, if any.
pub fn error_inner_message(error: &VsError) -> Option<&str> {
    match error {
        VsError::Cancelled | VsError::Io(_) | VsError::Other(_) => None,
        VsError::NotSupported(s)
        | VsError::NotFound(s)
        | VsError::NotImplemented(s)
        | VsError::IllegalArgument(s)
        | VsError::IllegalState(s)
        | VsError::ReadOnly(s)
        | VsError::PermissionDenied(s)
        | VsError::User(s) => Some(s.as_str()),
    }
}

/// Map a VsError to a different variant while preserving the inner message.
///
/// For variants without a `String` payload (`Cancelled`, `Io`, `Other`) the
/// error is returned unchanged.
pub fn map_error_variant(error: VsError, f: impl FnOnce(String) -> VsError) -> VsError {
    match error {
        VsError::Cancelled | VsError::Io(_) | VsError::Other(_) => error,
        VsError::NotSupported(s)
        | VsError::NotFound(s)
        | VsError::NotImplemented(s)
        | VsError::IllegalArgument(s)
        | VsError::IllegalState(s)
        | VsError::ReadOnly(s)
        | VsError::PermissionDenied(s)
        | VsError::User(s) => f(s),
    }
}

/// Count errors by variant name in an accumulator.
pub fn count_by_variant(acc: &ErrorAccumulator) -> std::collections::HashMap<&'static str, usize> {
    let mut counts = std::collections::HashMap::new();
    for record in acc.records() {
        let key = record.error.code();
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

/// Return the most recent error record from an accumulator, if any.
pub fn most_recent_record(acc: &ErrorAccumulator) -> Option<&ErrorRecord> {
    acc.records().last()
}


// ---------------------------------------------------------------------------
// Error classification and filtering utilities
// ---------------------------------------------------------------------------

/// Return true if the error variant represents a permissions/access issue.
pub fn is_access_error(error: &VsError) -> bool {
    matches!(error, VsError::PermissionDenied(_) | VsError::ReadOnly(_))
}

/// Return true if the error is retryable (not a permanent logic error).
pub fn is_retryable(error: &VsError) -> bool {
    matches!(error, VsError::Io(_) | VsError::Other(_))
}

/// Wrap a string message into a `VsError::User` variant.
pub fn user_error(msg: impl Into<String>) -> VsError {
    VsError::User(msg.into())
}

/// Wrap a string into a `VsError::IllegalArgument` variant.
pub fn illegal_argument(msg: impl Into<String>) -> VsError {
    VsError::IllegalArgument(msg.into())
}

/// Wrap a string into a `VsError::IllegalState` variant.
pub fn illegal_state(msg: impl Into<String>) -> VsError {
    VsError::IllegalState(msg.into())
}

/// Filter error records by minimum severity level.
pub fn filter_by_severity(acc: &ErrorAccumulator, min: ErrorSeverity) -> Vec<&ErrorRecord> {
    let min_ord = severity_ordinal(min);
    acc.records()
        .iter()
        .filter(|r| severity_ordinal(r.severity()) >= min_ord)
        .collect()
}

/// Return a numeric ordinal for severity comparison.
fn severity_ordinal(s: ErrorSeverity) -> u8 {
    match s {
        ErrorSeverity::Info => 0,
        ErrorSeverity::Warning => 1,
        ErrorSeverity::Error => 2,
    }
}

/// Partition accumulator records into errors and non-errors.
pub fn partition_by_severity(acc: &ErrorAccumulator) -> (Vec<&ErrorRecord>, Vec<&ErrorRecord>) {
    let errors: Vec<&ErrorRecord> = acc
        .records()
        .iter()
        .filter(|r| matches!(r.severity(), ErrorSeverity::Error))
        .collect();
    let non_errors: Vec<&ErrorRecord> = acc
        .records()
        .iter()
        .filter(|r| !matches!(r.severity(), ErrorSeverity::Error))
        .collect();
    (errors, non_errors)
}

/// Produce a formatted report of all errors in an accumulator.
pub fn error_report(acc: &ErrorAccumulator) -> String {
    if acc.is_empty() {
        return "No errors recorded.".to_string();
    }
    let mut report = format!("{} error(s) recorded:\n", acc.len());
    for (i, rec) in acc.records().iter().enumerate() {
        report.push_str(&format!("  {}. {}\n", i + 1, rec));
    }
    report
}

/// Extract all unique error codes from an accumulator.
pub fn unique_error_codes(acc: &ErrorAccumulator) -> Vec<&'static str> {
    let mut codes: Vec<&'static str> = acc.records().iter().map(|r| r.error.code()).collect();
    codes.sort();
    codes.dedup();
    codes
}

/// Returns true if the error is a "not found" variant.
pub fn is_not_found(error: &VsError) -> bool {
    matches!(error, VsError::NotFound(_))
}

/// Returns true if the error is a "not implemented" variant.
pub fn is_not_implemented(error: &VsError) -> bool {
    matches!(error, VsError::NotImplemented(_))
}

/// Chain two errors: if `primary` is a cancellation, return `fallback` instead.
pub fn or_on_cancel(primary: VsError, fallback: VsError) -> VsError {
    if is_cancelled(&primary) {
        fallback
    } else {
        primary
    }
}

// ---------------------------------------------------------------------------
// ErrorReporter – aggregates errors and produces reports
// ---------------------------------------------------------------------------

/// Aggregates [`ErrorRecord`]s and produces summary reports.
pub struct ErrorReporter {
    errors: Vec<ErrorRecord>,
    max_errors: usize,
}

impl ErrorReporter {
    /// Creates a new reporter that will hold at most `max_errors` records.
    pub fn new(max_errors: usize) -> Self {
        Self {
            errors: Vec::new(),
            max_errors,
        }
    }

    /// Reports an error with the given severity.
    pub fn report(&mut self, error: VsError, severity: ErrorSeverity) {
        if self.errors.len() < self.max_errors {
            let record = ErrorRecord::new(error).with_severity(severity);
            self.errors.push(record);
        }
    }

    /// Reports an error with severity and a context string as source location.
    pub fn report_with_context(
        &mut self,
        error: VsError,
        severity: ErrorSeverity,
        context: &str,
    ) {
        if self.errors.len() < self.max_errors {
            let record = ErrorRecord::new(error)
                .with_severity(severity)
                .with_source_location(context);
            self.errors.push(record);
        }
    }

    /// Number of records with [`ErrorSeverity::Error`].
    pub fn error_count(&self) -> usize {
        self.errors
            .iter()
            .filter(|r| r.severity() == ErrorSeverity::Error)
            .count()
    }

    /// Number of records with [`ErrorSeverity::Warning`].
    pub fn warning_count(&self) -> usize {
        self.errors
            .iter()
            .filter(|r| r.severity() == ErrorSeverity::Warning)
            .count()
    }

    /// Returns `true` if any record has [`ErrorSeverity::Error`].
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Produces a human-readable summary like `"N errors, M warnings, K info"`.
    pub fn summary(&self) -> String {
        let errors = self.error_count();
        let warnings = self.warning_count();
        let info = self.errors.len() - errors - warnings;
        format!("{errors} errors, {warnings} warnings, {info} info")
    }

    /// Removes all recorded errors.
    pub fn clear(&mut self) {
        self.errors.clear();
    }

    /// Returns `true` if the reporter has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.errors.len() >= self.max_errors
    }
}

// ---------------------------------------------------------------------------
// ErrorRecoveryStrategy – defines recovery strategies
// ---------------------------------------------------------------------------

/// Describes how to recover from a given error.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorRecoveryStrategy {
    /// Retry the operation up to `max_attempts` times with `delay_ms` between.
    Retry { max_attempts: u32, delay_ms: u64 },
    /// Use a fallback value instead.
    Fallback { fallback_value: String },
    /// Silently ignore the error.
    Ignore,
    /// Abort the operation immediately.
    Abort,
}

impl ErrorRecoveryStrategy {
    /// Suggests a recovery strategy based on the error variant.
    pub fn suggest_for(error: &VsError) -> Self {
        match error {
            VsError::Cancelled => Self::Ignore,
            VsError::PermissionDenied(_) | VsError::ReadOnly(_) => Self::Abort,
            VsError::NotFound(_) => Self::Fallback {
                fallback_value: String::new(),
            },
            VsError::Io(_) | VsError::IllegalState(_) => Self::Retry {
                max_attempts: 3,
                delay_ms: 1000,
            },
            _ => Self::Retry {
                max_attempts: 1,
                delay_ms: 500,
            },
        }
    }

    /// Returns `true` if the strategy involves retrying.
    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::Retry { .. })
    }

    /// Human-readable description of the strategy.
    pub fn description(&self) -> String {
        match self {
            Self::Retry {
                max_attempts,
                delay_ms,
            } => format!("Retry up to {max_attempts} times with {delay_ms}ms delay"),
            Self::Fallback { fallback_value } => {
                if fallback_value.is_empty() {
                    "Use default fallback value".to_owned()
                } else {
                    format!("Fallback to: {fallback_value}")
                }
            }
            Self::Ignore => "Ignore the error".to_owned(),
            Self::Abort => "Abort the operation".to_owned(),
        }
    }

    /// Number of retry attempts, or `0` for non-retry strategies.
    pub fn max_attempts(&self) -> u32 {
        match self {
            Self::Retry { max_attempts, .. } => *max_attempts,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// UserFacingErrorFormatter – formats errors for display to users
// ---------------------------------------------------------------------------

/// Formats errors into user-friendly messages without internal details.
pub struct UserFacingErrorFormatter;

impl UserFacingErrorFormatter {
    pub fn new() -> Self {
        Self
    }

    /// Returns a user-friendly message for the given error.
    pub fn format(&self, error: &VsError) -> String {
        match error {
            VsError::Cancelled => "The operation was cancelled.".to_owned(),
            VsError::NotSupported(s) => format!("This feature is not supported: {s}"),
            VsError::NotFound(s) => format!("Could not find: {s}"),
            VsError::NotImplemented(s) => format!("Not yet available: {s}"),
            VsError::IllegalArgument(s) => format!("Invalid input: {s}"),
            VsError::IllegalState(s) => format!("Unexpected state: {s}"),
            VsError::ReadOnly(s) => format!("Cannot modify read-only resource: {s}"),
            VsError::PermissionDenied(s) => format!("Access denied: {s}"),
            VsError::User(s) => s.clone(),
            VsError::Io(_) => "A system I/O error occurred.".to_owned(),
            VsError::Other(_) => "An unexpected error occurred.".to_owned(),
        }
    }

    /// Returns a user-friendly message with a recovery suggestion appended.
    pub fn format_with_suggestion(&self, error: &VsError) -> String {
        let msg = self.format(error);
        let suggestion = recovery_suggestion(error);
        format!("{msg}\nSuggestion: {suggestion}")
    }

    /// Formats an [`ErrorChain`] as a numbered list.
    pub fn format_chain(&self, chain: &ErrorChain) -> String {
        let mut out = String::new();
        for (i, (ctx, error)) in chain.errors.iter().enumerate() {
            if !out.is_empty() {
                out.push('\n');
            }
            let msg = self.format(error);
            if ctx.is_empty() {
                out.push_str(&format!("{}. {msg}", i + 1));
            } else {
                out.push_str(&format!("{}. [{ctx}] {msg}", i + 1));
            }
        }
        out
    }

    /// Returns an icon character for the given severity.
    pub fn severity_icon(severity: ErrorSeverity) -> &'static str {
        match severity {
            ErrorSeverity::Error => "❌",
            ErrorSeverity::Warning => "⚠️",
            ErrorSeverity::Info => "ℹ️",
        }
    }
}

// ---------------------------------------------------------------------------
// ErrorContextBuilder – builds error context chains
// ---------------------------------------------------------------------------

/// Builder for composing hierarchical error context strings.
pub struct ErrorContextBuilder {
    contexts: Vec<String>,
}

impl ErrorContextBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self {
            contexts: Vec::new(),
        }
    }

    /// Appends a context and returns the builder (for chaining).
    pub fn with(mut self, ctx: impl Into<String>) -> Self {
        self.contexts.push(ctx.into());
        self
    }

    /// Joins all contexts with `" -> "`.
    pub fn build_message(&self) -> String {
        self.contexts.join(" -> ")
    }

    /// Number of context segments.
    pub fn depth(&self) -> usize {
        self.contexts.len()
    }

    /// Appends a context in-place.
    pub fn push(&mut self, ctx: impl Into<String>) {
        self.contexts.push(ctx.into());
    }

    /// Returns `true` if no context has been added.
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ErrorBoundaryHandler - error boundary handler
// ---------------------------------------------------------------------------

/// Severity level for error boundary handler issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorBoundaryHandlerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ErrorBoundaryHandlerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ErrorBoundaryHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorBoundaryHandlerEntry {
    pub id: String,
    pub label: String,
    pub severity: ErrorBoundaryHandlerSeverity,
    pub detail: Option<String>,
    pub error_count: usize,
    enabled: bool,
}

impl ErrorBoundaryHandlerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ErrorBoundaryHandlerSeverity::Low,
            detail: None,
            error_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ErrorBoundaryHandlerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_error_count(mut self, val: usize) -> Self {
        self.error_count = val;
        self
    }

    pub fn has_errors(&self) -> bool {
        self.enabled && self.severity >= ErrorBoundaryHandlerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.error_count, det)
    }
}

impl fmt::Display for ErrorBoundaryHandlerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ErrorBoundaryHandlerEntry] items.
#[derive(Debug, Clone)]
pub struct ErrorBoundaryHandler {
    entries: Vec<ErrorBoundaryHandlerEntry>,
    name: String,
    capacity: usize,
}

impl ErrorBoundaryHandler {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ErrorBoundaryHandlerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ErrorBoundaryHandlerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ErrorBoundaryHandlerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn error_count(&self) -> usize { self.entries.len() }

    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|e| e.has_errors())
    }

    pub fn entries_by_severity(&self, severity: ErrorBoundaryHandlerSeverity) -> Vec<&ErrorBoundaryHandlerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ErrorBoundaryHandlerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ErrorBoundaryHandlerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ErrorBoundaryHandlerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ErrorDialogFormatter - error dialog formatter
// ---------------------------------------------------------------------------

/// Configuration for [ErrorDialogFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDialogFormatterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub boundary_depth: usize,
}

impl ErrorDialogFormatterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, boundary_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_boundary_depth(mut self, val: usize) -> Self { self.boundary_depth = val; self }
}

impl Default for ErrorDialogFormatterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ErrorDialogFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDialogFormatterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ErrorDialogFormatterItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_recoverable(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ErrorDialogFormatterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ErrorDialogFormatterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ErrorDialogFormatter {
    config: ErrorDialogFormatterConfig,
    items: Vec<ErrorDialogFormatterItem>,
}

impl ErrorDialogFormatter {
    pub fn new(config: ErrorDialogFormatterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ErrorDialogFormatterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ErrorDialogFormatterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ErrorDialogFormatterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn boundary_depth(&self) -> usize { self.items.len() }

    pub fn is_recoverable(&self) -> bool {
        self.items.iter().any(|i| i.is_recoverable())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ErrorDialogFormatterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ErrorDialogFormatterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ErrorDialogFormatterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// errors – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XErrorsValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XErrorsValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XErrorsTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XErrorsTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_errors_validate_string(value: &str, max_len: usize) -> XErrorsValidationResult {
    if value.is_empty() {
        return XErrorsValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XErrorsValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XErrorsValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_errors_validate_range(value: i64, min: i64, max: i64) -> XErrorsValidationResult {
    if value < min || value > max {
        XErrorsValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XErrorsValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_errors_filter_by_tag<'a>(
    entries: &'a [XErrorsTaggedEntry],
    tag: &str,
) -> Vec<&'a XErrorsTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_errors_group_by_tag(
    entries: &[XErrorsTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XErrorsTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_errors_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_errors_dedup_entries(entries: Vec<XErrorsTaggedEntry>) -> Vec<XErrorsTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
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

    #[test]
    fn format_error_chain_multiline() {
        let chain = ErrorChain::new(VsError::NotFound("file.txt".into()))
            .with_context("loading config", VsError::IllegalState("missing config".into()));
        let formatted = format_error_chain(&chain);
        assert!(formatted.contains("NOT_FOUND"));
        assert!(formatted.contains("loading config"));
        assert!(formatted.lines().count() >= 2);
    }

    #[test]
    fn format_error_chain_oneline_joins() {
        let chain = ErrorChain::new(VsError::Cancelled)
            .with_context("retry", VsError::Cancelled);
        let oneline = format_error_chain_oneline(&chain);
        assert!(oneline.contains(" -> "));
    }

    #[test]
    fn error_rate_tracker_records_and_counts() {
        let mut tracker = ErrorRateTracker::new(5, 10);
        tracker.record_error(0);
        tracker.record_error(5);
        tracker.record_error(15);
        assert_eq!(tracker.total_errors(), 3);
        assert!((tracker.average_rate() - 0.6).abs() < 0.01);
    }

    #[test]
    fn enriched_error_format_with_context() {
        let enriched = EnrichedError::new(VsError::NotFound("data.json".into()))
            .with("component", "loader")
            .with("attempt", "3");
        let out = enriched.format();
        assert!(out.contains("Not found: data.json"));
        assert!(out.contains("component=loader"));
        assert!(out.contains("attempt=3"));
    }

    #[test]
    fn recovery_suggestion_gives_advice() {
        let suggestion = recovery_suggestion(&VsError::PermissionDenied("secret".into()));
        assert!(suggestion.contains("Permission"));
        let suggestion2 = recovery_suggestion(&VsError::Cancelled);
        assert!(suggestion2.contains("try again"));
    }

    #[test]
    fn requires_user_action_classifies_correctly() {
        assert!(requires_user_action(&VsError::NotFound("x".into())));
        assert!(requires_user_action(&VsError::User("oops".into())));
        assert!(!requires_user_action(&VsError::Cancelled));
        assert!(!requires_user_action(&VsError::IllegalState("bad".into())));
    }

    #[test]
    fn is_transient_timeout() {
        assert!(is_transient(&VsError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "slow",
        ))));
    }

    #[test]
    fn is_transient_not_found() {
        assert!(!is_transient(&VsError::NotFound("x".into())));
    }

    #[test]
    fn is_permission_error_denied() {
        assert!(is_permission_error(&VsError::PermissionDenied("no".into())));
        assert!(is_permission_error(&VsError::ReadOnly("ro".into())));
    }

    #[test]
    fn is_permission_error_other() {
        assert!(!is_permission_error(&VsError::Cancelled));
        assert!(!is_permission_error(&VsError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "disk",
        ))));
    }

    #[test]
    fn error_inner_message_some() {
        let e = VsError::NotFound("file.txt".into());
        assert_eq!(error_inner_message(&e), Some("file.txt"));
    }

    #[test]
    fn error_inner_message_cancelled() {
        assert_eq!(error_inner_message(&VsError::Cancelled), None);
    }

    #[test]
    fn map_error_variant_preserves_message() {
        let e = VsError::NotFound("disk full".into());
        let mapped = map_error_variant(e, |s| VsError::User(s));
        assert_eq!(error_inner_message(&mapped), Some("disk full"));
    }

    #[test]
    fn map_error_variant_cancelled_unchanged() {
        let e = VsError::Cancelled;
        let mapped = map_error_variant(e, |s| VsError::User(s));
        assert!(is_cancelled(&mapped));
    }

    #[test]
    fn count_by_variant_counts() {
        let mut acc = ErrorAccumulator::new();
        acc.push(VsError::NotFound("a".into()));
        acc.push(VsError::NotFound("b".into()));
        acc.push(VsError::User("c".into()));
        let counts = count_by_variant(&acc);
        assert_eq!(counts["NOT_FOUND"], 2);
        assert_eq!(counts["USER_ERROR"], 1);
    }

    #[test]
    fn most_recent_record_empty() {
        let acc = ErrorAccumulator::new();
        assert!(most_recent_record(&acc).is_none());
    }

    #[test]
    fn most_recent_record_returns_last() {
        let mut acc = ErrorAccumulator::new();
        acc.push(VsError::NotFound("first".into()));
        acc.push(VsError::User("second".into()));
        let rec = most_recent_record(&acc).unwrap();
        assert_eq!(error_inner_message(&rec.error), Some("second"));
    }

    #[test]
    fn is_access_error_checks() {
        assert!(is_access_error(&VsError::PermissionDenied("denied".into())));
        assert!(is_access_error(&VsError::ReadOnly("file".into())));
        assert!(!is_access_error(&VsError::NotFound("missing".into())));
        assert!(!is_access_error(&VsError::Cancelled));
    }

    #[test]
    fn is_retryable_checks() {
        let io_err = VsError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"));
        assert!(is_retryable(&io_err));
        assert!(!is_retryable(&VsError::Cancelled));
        assert!(!is_retryable(&VsError::NotFound("x".into())));
    }

    #[test]
    fn convenience_constructors() {
        let u = user_error("oops");
        assert_eq!(u.to_string(), "oops");
        let ia = illegal_argument("bad arg");
        assert_eq!(ia.to_string(), "Illegal argument: bad arg");
        let is = illegal_state("bad state");
        assert_eq!(is.to_string(), "Illegal state: bad state");
    }

    #[test]
    fn filter_by_severity_filters() {
        let mut acc = ErrorAccumulator::new();
        acc.push_record(ErrorRecord::new(VsError::User("info".into())).with_severity(ErrorSeverity::Info));
        acc.push_record(ErrorRecord::new(VsError::User("warn".into())).with_severity(ErrorSeverity::Warning));
        acc.push_record(ErrorRecord::new(VsError::User("err".into())).with_severity(ErrorSeverity::Error));
        let warnings_up = filter_by_severity(&acc, ErrorSeverity::Warning);
        assert_eq!(warnings_up.len(), 2);
        let errors_only = filter_by_severity(&acc, ErrorSeverity::Error);
        assert_eq!(errors_only.len(), 1);
    }

    #[test]
    fn partition_by_severity_splits() {
        let mut acc = ErrorAccumulator::new();
        acc.push_record(ErrorRecord::new(VsError::User("err".into())).with_severity(ErrorSeverity::Error));
        acc.push_record(ErrorRecord::new(VsError::User("info".into())).with_severity(ErrorSeverity::Info));
        let (errors, non_errors) = partition_by_severity(&acc);
        assert_eq!(errors.len(), 1);
        assert_eq!(non_errors.len(), 1);
    }

    #[test]
    fn error_report_format() {
        let acc = ErrorAccumulator::new();
        assert_eq!(error_report(&acc), "No errors recorded.");
        let mut acc2 = ErrorAccumulator::new();
        acc2.push(VsError::NotFound("a".into()));
        acc2.push(VsError::User("b".into()));
        let report = error_report(&acc2);
        assert!(report.contains("2 error(s)"));
    }

    #[test]
    fn unique_error_codes_deduplicates() {
        let mut acc = ErrorAccumulator::new();
        acc.push(VsError::NotFound("a".into()));
        acc.push(VsError::NotFound("b".into()));
        acc.push(VsError::User("c".into()));
        let codes = unique_error_codes(&acc);
        assert_eq!(codes.len(), 2);
    }

    #[test]
    fn is_not_found_and_not_implemented() {
        assert!(is_not_found(&VsError::NotFound("x".into())));
        assert!(!is_not_found(&VsError::Cancelled));
        assert!(is_not_implemented(&VsError::NotImplemented("x".into())));
        assert!(!is_not_implemented(&VsError::Cancelled));
    }

    #[test]
    fn or_on_cancel_replaces() {
        let primary = VsError::Cancelled;
        let fallback = VsError::User("fallback".into());
        let result = or_on_cancel(primary, fallback);
        assert_eq!(result.to_string(), "fallback");
        let primary2 = VsError::NotFound("x".into());
        let fallback2 = VsError::User("y".into());
        let result2 = or_on_cancel(primary2, fallback2);
        assert_eq!(result2.to_string(), "Not found: x");
    }

    #[test]
    fn test_reporter_add_errors() {
        let mut r = ErrorReporter::new(10);
        r.report(VsError::NotFound("a".into()), ErrorSeverity::Error);
        r.report(VsError::Cancelled, ErrorSeverity::Info);
        assert_eq!(r.error_count(), 1);
    }

    #[test]
    fn test_reporter_summary() {
        let mut r = ErrorReporter::new(10);
        r.report(VsError::NotFound("a".into()), ErrorSeverity::Error);
        r.report(VsError::Cancelled, ErrorSeverity::Warning);
        r.report(VsError::User("x".into()), ErrorSeverity::Info);
        assert_eq!(r.summary(), "1 errors, 1 warnings, 1 info");
    }

    #[test]
    fn test_reporter_max_errors() {
        let mut r = ErrorReporter::new(2);
        r.report(VsError::Cancelled, ErrorSeverity::Error);
        r.report(VsError::Cancelled, ErrorSeverity::Error);
        r.report(VsError::Cancelled, ErrorSeverity::Error);
        assert!(r.is_full());
        assert_eq!(r.error_count(), 2);
    }

    #[test]
    fn test_reporter_clear() {
        let mut r = ErrorReporter::new(10);
        r.report(VsError::Cancelled, ErrorSeverity::Error);
        assert!(r.has_errors());
        r.clear();
        assert!(!r.has_errors());
        assert_eq!(r.summary(), "0 errors, 0 warnings, 0 info");
    }

    #[test]
    fn test_recovery_suggest_cancelled() {
        let strategy = ErrorRecoveryStrategy::suggest_for(&VsError::Cancelled);
        assert_eq!(strategy, ErrorRecoveryStrategy::Ignore);
    }

    #[test]
    fn test_recovery_suggest_timeout() {
        let strategy =
            ErrorRecoveryStrategy::suggest_for(&VsError::IllegalState("timeout".into()));
        assert_eq!(
            strategy,
            ErrorRecoveryStrategy::Retry {
                max_attempts: 3,
                delay_ms: 1000,
            }
        );
    }

    #[test]
    fn test_recovery_suggest_not_found() {
        let strategy = ErrorRecoveryStrategy::suggest_for(&VsError::NotFound("x".into()));
        assert_eq!(
            strategy,
            ErrorRecoveryStrategy::Fallback {
                fallback_value: String::new()
            }
        );
    }

    #[test]
    fn test_recovery_is_retriable() {
        assert!(ErrorRecoveryStrategy::Retry {
            max_attempts: 1,
            delay_ms: 100,
        }
        .is_retriable());
        assert!(!ErrorRecoveryStrategy::Abort.is_retriable());
        assert!(!ErrorRecoveryStrategy::Ignore.is_retriable());
    }

    #[test]
    fn test_recovery_description() {
        let s = ErrorRecoveryStrategy::Retry {
            max_attempts: 3,
            delay_ms: 1000,
        };
        assert_eq!(s.description(), "Retry up to 3 times with 1000ms delay");
        assert_eq!(ErrorRecoveryStrategy::Abort.description(), "Abort the operation");
        assert_eq!(ErrorRecoveryStrategy::Abort.max_attempts(), 0);
    }

    #[test]
    fn test_user_formatter_basic() {
        let f = UserFacingErrorFormatter::new();
        assert_eq!(f.format(&VsError::Cancelled), "The operation was cancelled.");
        assert_eq!(
            f.format(&VsError::NotFound("file.txt".into())),
            "Could not find: file.txt"
        );
        assert_eq!(
            UserFacingErrorFormatter::severity_icon(ErrorSeverity::Error),
            "❌"
        );
    }

    #[test]
    fn test_user_formatter_with_suggestion() {
        let f = UserFacingErrorFormatter::new();
        let msg = f.format_with_suggestion(&VsError::Cancelled);
        assert!(msg.contains("cancelled"));
        assert!(msg.contains("Suggestion:"));
    }

    #[test]
    fn test_context_builder_chain() {
        let builder = ErrorContextBuilder::new()
            .with("open file")
            .with("parse header")
            .with("validate checksum");
        assert_eq!(builder.depth(), 3);
        assert!(!builder.is_empty());
        assert_eq!(
            builder.build_message(),
            "open file -> parse header -> validate checksum"
        );

        let empty = ErrorContextBuilder::new();
        assert!(empty.is_empty());
        assert_eq!(empty.build_message(), "");
    }


#[test]
    fn errorboundaryhandler_severity_ordering() {
        assert!(ErrorBoundaryHandlerSeverity::Critical > ErrorBoundaryHandlerSeverity::High);
        assert!(ErrorBoundaryHandlerSeverity::High > ErrorBoundaryHandlerSeverity::Medium);
        assert!(ErrorBoundaryHandlerSeverity::Medium > ErrorBoundaryHandlerSeverity::Low);
    }

    #[test]
    fn errorboundaryhandler_severity_display() {
        assert_eq!(ErrorBoundaryHandlerSeverity::Low.to_string(), "low");
        assert_eq!(ErrorBoundaryHandlerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn errorboundaryhandler_entry_creation() {
        let e = ErrorBoundaryHandlerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ErrorBoundaryHandlerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn errorboundaryhandler_entry_builder() {
        let e = ErrorBoundaryHandlerEntry::new("e2", "Entry 2")
            .with_severity(ErrorBoundaryHandlerSeverity::High)
            .with_detail("some detail")
            .with_error_count(42);
        assert_eq!(e.severity, ErrorBoundaryHandlerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.error_count, 42);
    }

    #[test]
    fn errorboundaryhandler_entry_enable_disable() {
        let mut e = ErrorBoundaryHandlerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn errorboundaryhandler_add_and_count() {
        let mut mgr = ErrorBoundaryHandler::new("test");
        mgr.add(ErrorBoundaryHandlerEntry::new("a", "A"));
        mgr.add(ErrorBoundaryHandlerEntry::new("b", "B").with_severity(ErrorBoundaryHandlerSeverity::High));
        assert_eq!(mgr.error_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn errorboundaryhandler_remove() {
        let mut mgr = ErrorBoundaryHandler::new("test");
        mgr.add(ErrorBoundaryHandlerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn errorboundaryhandler_capacity() {
        let mut mgr = ErrorBoundaryHandler::new("test").with_capacity(1);
        assert!(mgr.add(ErrorBoundaryHandlerEntry::new("a", "A")));
        assert!(!mgr.add(ErrorBoundaryHandlerEntry::new("b", "B")));
    }

    #[test]
    fn errorboundaryhandler_sorted_by_severity() {
        let mut mgr = ErrorBoundaryHandler::new("test");
        mgr.add(ErrorBoundaryHandlerEntry::new("lo", "Low"));
        mgr.add(ErrorBoundaryHandlerEntry::new("hi", "High").with_severity(ErrorBoundaryHandlerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ErrorBoundaryHandlerSeverity::Critical);
    }

    #[test]
    fn errorboundaryhandler_summary() {
        let mgr = ErrorBoundaryHandler::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn errordialogformatter_config_defaults() {
        let cfg = ErrorDialogFormatterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn errordialogformatter_item_creation() {
        let item = ErrorDialogFormatterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn errordialogformatter_add_and_get() {
        let mut mgr = ErrorDialogFormatter::new(ErrorDialogFormatterConfig::new("test"));
        mgr.add(ErrorDialogFormatterItem::new("k1", "v1"));
        assert_eq!(mgr.boundary_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn errordialogformatter_remove_item() {
        let mut mgr = ErrorDialogFormatter::new(ErrorDialogFormatterConfig::new("test"));
        mgr.add(ErrorDialogFormatterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn errordialogformatter_sorted_by_priority() {
        let mut mgr = ErrorDialogFormatter::new(ErrorDialogFormatterConfig::new("test"));
        mgr.add(ErrorDialogFormatterItem::new("lo", "low").with_priority(1));
        mgr.add(ErrorDialogFormatterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn errordialogformatter_items_with_tag() {
        let mut mgr = ErrorDialogFormatter::new(ErrorDialogFormatterConfig::new("test"));
        mgr.add(ErrorDialogFormatterItem::new("a", "1").with_tag("x"));
        mgr.add(ErrorDialogFormatterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn errordialogformatter_report() {
        let mgr = ErrorDialogFormatter::new(ErrorDialogFormatterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- errors additional tests -------------------------------------------

    #[test]
    fn x_errors_validation_ok() {
        let r = x_errors_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_errors_validation_empty() {
        let r = x_errors_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_errors_validation_too_long() {
        let r = x_errors_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_errors_validate_range_ok() {
        assert!(x_errors_validate_range(5, 1, 10).is_ok());
        assert!(x_errors_validate_range(1, 1, 10).is_ok());
        assert!(x_errors_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_errors_validate_range_out() {
        assert!(!x_errors_validate_range(0, 1, 10).is_ok());
        assert!(!x_errors_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_errors_tagged_entry_basic() {
        let e = XErrorsTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_errors_tagged_entry_with_tag() {
        let e = XErrorsTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_errors_filter_by_tag_basic() {
        let entries = vec![
            XErrorsTaggedEntry::new("a", "1").with_tag("x"),
            XErrorsTaggedEntry::new("b", "2").with_tag("y"),
            XErrorsTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_errors_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_errors_group_by_tag_basic() {
        let entries = vec![
            XErrorsTaggedEntry::new("a", "1").with_tag("x"),
            XErrorsTaggedEntry::new("b", "2"),
            XErrorsTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_errors_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_errors_djb2_hash_deterministic() {
        let h1 = x_errors_djb2_hash("hello");
        let h2 = x_errors_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_errors_djb2_hash("hello"), x_errors_djb2_hash("world"));
    }

    #[test]
    fn x_errors_dedup_entries_basic() {
        let entries = vec![
            XErrorsTaggedEntry::new("a", "1"),
            XErrorsTaggedEntry::new("a", "2"),
            XErrorsTaggedEntry::new("b", "3"),
        ];
        let deduped = x_errors_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_errors_validation_result_warning() {
        let w = XErrorsValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_errors_filter_by_tag_empty() {
        let entries: Vec<XErrorsTaggedEntry> = vec![];
        assert!(x_errors_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_errors_tagged_entry_no_tag_match() {
        let e = XErrorsTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }

}
