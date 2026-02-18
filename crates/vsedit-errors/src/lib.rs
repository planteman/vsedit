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



// ---------------------------------------------------------------------------
// errors – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for error handling infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YErrorsErrorRecoveryMode {
    Ignore,
    Retry,
    Fallback,
    Abort,
}

impl YErrorsErrorRecoveryMode {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Ignore => 0,
            Self::Retry => 1,
            Self::Fallback => 2,
            Self::Abort => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ignore => "Ignore",
            Self::Retry => "Retry",
            Self::Fallback => "Fallback",
            Self::Abort => "Abort",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YErrorsErrorRecoveryMode] {
        &[
            YErrorsErrorRecoveryMode::Ignore,
            YErrorsErrorRecoveryMode::Retry,
            YErrorsErrorRecoveryMode::Fallback,
            YErrorsErrorRecoveryMode::Abort,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YErrorsErrorRecoveryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks error log data.
#[derive(Debug, Clone)]
pub struct YErrorsErrorLog {
    pub errors: Vec<(String, u64)>,
    pub max_entries: usize,
    pub overflow: bool,
}

impl YErrorsErrorLog {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            max_entries: 0,
            overflow: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.errors.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YErrorsErrorLog({}: {:?})", "errors", self.errors)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_errors_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_errors_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_errors_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_errors_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_errors_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_errors_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_errors_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_errors_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// errors – Extended error aggregator helpers
// ---------------------------------------------------------------------------

/// Priority levels for error aggregator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZErrorsPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZErrorsPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZErrorsPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZErrorsPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks error aggregator data.
#[derive(Debug, Clone)]
pub struct ZErrorsErrorAggregator {
    pub buckets: Vec<(String, usize)>,
    pub window_sec: u64,
    pub overflow: bool,
}

impl ZErrorsErrorAggregator {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            buckets: Vec::new(),
            window_sec: 0,
            overflow: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.buckets.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZErrorsErrorAggregator[window_sec={:?}, overflow={:?}]", self.window_sec, self.overflow)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.overflow = !c.overflow;
        c
    }
}

/// Compute a simple rolling hash for error aggregator.
pub fn z_errors_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_errors_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_errors_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_errors_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_errors_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_errors_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_errors_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 74
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer74 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer74 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_74(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_74<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_74<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_74(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_74(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 45
// ---------------------------------------------------------------------------

/// Generic object pool `Xc45Pool<T>`.
pub struct Xc45Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc45Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc45PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc45Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc45PoolStats {
        Xc45PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc45Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc45Scheduler`.
pub struct Xc45Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc45Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc45Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_45 hash for the given byte slice.
pub fn xc_45_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_45 convention.
pub fn xc_45_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe87 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe87Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe87PipelineError {
    pub stage: Xe87Stage,
    pub message: String,
}

impl std::fmt::Display for Xe87PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe87Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe87Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError>>>,
    stage_names: Vec<Xe87Stage>,
}

impl Xe87Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe87Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe87Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe87Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe87Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe87Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe87CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe87CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe87Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe87CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe87CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe87Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe87CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_87_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe87CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_87_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe87CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_87_pipeline_identity(data: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
    Ok(data)
}

pub fn xe_87_pipeline_double(data: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_87_pipeline_reverse(data: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_87_pipeline_filter_zeros(data: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_87_pipeline_fail(_data: Vec<u8>) -> std::result::Result<Vec<u8>, Xe87PipelineError> {
    Err(Xe87PipelineError {
        stage: Xe87Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_85: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg85Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg85Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg85Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_85: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg85Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg85Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg85Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg85Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 44).
pub struct Xh44SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh44SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 86 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 44).
pub struct Xh44BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh44BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 44).
pub struct Xi44Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi44Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi44Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi44Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 44).
pub struct Xi44IntervalTree {
    xi_intervals: Vec<Xi44Interval>,
}

impl Xi44IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi44Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi44Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi44Interval) -> Vec<&Xi44Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi44Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi44Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi44Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi44Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi44Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi44Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 44) ---

/// Disjoint set / union-find for crate 44.
pub struct Xj44UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj44UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ44_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 44.
pub struct Xj44BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj44BTreeNode<K, V>>>,
    len: usize,
}

struct Xj44BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj44BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj44BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ44_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ44_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj44BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj44BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj44BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj44BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_44 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk44SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk44SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk44DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk44DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_44).
#[derive(Debug, Clone)]
pub struct Xl44Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl44Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_44).
#[derive(Debug, Clone)]
pub struct Xl44SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl44SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm44MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm44MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm44Tokenizer {
    text: String,
}

impl Xm44Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
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


    // -- errors extended domain tests ----------------------------------------

    #[test]
    fn y_errors_enum_index() {
        assert_eq!(YErrorsErrorRecoveryMode::Ignore.index(), 0);
        assert_eq!(YErrorsErrorRecoveryMode::Retry.index(), 1);
        assert_eq!(YErrorsErrorRecoveryMode::Fallback.index(), 2);
        assert_eq!(YErrorsErrorRecoveryMode::Abort.index(), 3);
    }

    #[test]
    fn y_errors_enum_label() {
        assert_eq!(YErrorsErrorRecoveryMode::Ignore.label(), "Ignore");
        assert_eq!(YErrorsErrorRecoveryMode::Retry.label(), "Retry");
        assert_eq!(YErrorsErrorRecoveryMode::Fallback.label(), "Fallback");
        assert_eq!(YErrorsErrorRecoveryMode::Abort.label(), "Abort");
    }

    #[test]
    fn y_errors_enum_all() {
        let all = YErrorsErrorRecoveryMode::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_errors_enum_is_default() {
        assert!(YErrorsErrorRecoveryMode::Ignore.is_default());
        assert!(!YErrorsErrorRecoveryMode::Abort.is_default());
    }

    #[test]
    fn y_errors_enum_display() {
        assert_eq!(format!("{}", YErrorsErrorRecoveryMode::Ignore), "Ignore");
    }

    #[test]
    fn y_errors_struct_new() {
        let s = YErrorsErrorLog::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_errors_struct_clear() {
        let mut s = YErrorsErrorLog::new();
        s.errors.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_errors_fingerprint_deterministic() {
        let h1 = y_errors_fingerprint("hello");
        let h2 = y_errors_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_errors_fingerprint("a"), y_errors_fingerprint("b"));
    }

    #[test]
    fn y_errors_truncate_short() {
        assert_eq!(y_errors_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_errors_truncate_long() {
        let r = y_errors_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_errors_normalize_key_basic() {
        assert_eq!(y_errors_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_errors_split_path_basic() {
        let parts = y_errors_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_errors_count_occurrences_basic() {
        assert_eq!(y_errors_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_errors_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_errors_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_errors_in_range_basic() {
        assert!(y_errors_in_range(5, 1, 10));
        assert!(y_errors_in_range(1, 1, 10));
        assert!(y_errors_in_range(10, 1, 10));
        assert!(!y_errors_in_range(0, 1, 10));
        assert!(!y_errors_in_range(11, 1, 10));
    }

    #[test]
    fn y_errors_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_errors_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_errors_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_errors_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- errors Z-extended tests -----------------------------------------------

    #[test]
    fn z_errors_priority_weight() {
        assert_eq!(ZErrorsPriority::Idle.weight(), 0);
        assert_eq!(ZErrorsPriority::Normal.weight(), 2);
        assert_eq!(ZErrorsPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_errors_priority_label() {
        assert_eq!(ZErrorsPriority::Low.label(), "low");
        assert_eq!(ZErrorsPriority::High.label(), "high");
    }

    #[test]
    fn z_errors_priority_is_elevated() {
        assert!(!ZErrorsPriority::Normal.is_elevated());
        assert!(ZErrorsPriority::High.is_elevated());
        assert!(ZErrorsPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_errors_priority_display() {
        assert_eq!(format!("{}", ZErrorsPriority::Idle), "idle");
    }

    #[test]
    fn z_errors_priority_all_asc() {
        let all = ZErrorsPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZErrorsPriority::Idle);
        assert_eq!(all[4], ZErrorsPriority::Realtime);
    }

    #[test]
    fn z_errors_struct_new() {
        let s = ZErrorsErrorAggregator::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_errors_struct_toggled_clone() {
        let s = ZErrorsErrorAggregator::new();
        let t = s.toggled_clone();
        assert_ne!(s.overflow, t.overflow);
    }

    #[test]
    fn z_errors_rolling_hash_deterministic() {
        let h1 = z_errors_rolling_hash(b"test");
        let h2 = z_errors_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_errors_rolling_hash(b"a"), z_errors_rolling_hash(b"b"));
    }

    #[test]
    fn z_errors_pad_to_basic() {
        assert_eq!(z_errors_pad_to("hi", 5), "hi   ");
        assert_eq!(z_errors_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_errors_is_identifier_basic() {
        assert!(z_errors_is_identifier("foo_bar"));
        assert!(z_errors_is_identifier("abc123"));
        assert!(!z_errors_is_identifier(""));
        assert!(!z_errors_is_identifier("has space"));
    }

    #[test]
    fn z_errors_levenshtein_basic() {
        assert_eq!(z_errors_levenshtein("", ""), 0);
        assert_eq!(z_errors_levenshtein("abc", "abc"), 0);
        assert_eq!(z_errors_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_errors_unique_words_basic() {
        let w = z_errors_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_errors_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_errors_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_errors_common_prefix_basic() {
        assert_eq!(z_errors_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_errors_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_errors_struct_clear() {
        let mut s = ZErrorsErrorAggregator::new();
        s.buckets.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_errors_rolling_hash_empty() {
        let h = z_errors_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_74_push_and_len() {
        let mut rb = super::XbRingBuffer74::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_74_overwrite() {
        let mut rb = super::XbRingBuffer74::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_74_get_out_of_bounds() {
        let rb = super::XbRingBuffer74::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_74_drain_all() {
        let mut rb = super::XbRingBuffer74::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_74_peek_front_back() {
        let mut rb = super::XbRingBuffer74::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_74_clear() {
        let mut rb = super::XbRingBuffer74::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_74_capacity() {
        let rb = super::XbRingBuffer74::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_74_basic() {
        let h = super::xb_fnv1a_74(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_74(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_74_different_inputs() {
        let h1 = super::xb_fnv1a_74(b"abc");
        let h2 = super::xb_fnv1a_74(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_74_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_74(&data);
        let dec = super::xb_rle_decode_74(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_74_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_74(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_74(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_74_values() {
        assert!((super::xb_clamp_74(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_74(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_74(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_74_values() {
        assert!((super::xb_lerp_74(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_74(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_74(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_74_wrap_around_twice() {
        let mut rb = super::XbRingBuffer74::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 45 ----

    #[test]
    fn xc_45_pool_new_empty() {
        let pool: super::Xc45Pool<i32> = super::Xc45Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_45_pool_release_acquire() {
        let mut pool = super::Xc45Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_45_pool_acquire_empty() {
        let mut pool: super::Xc45Pool<i32> = super::Xc45Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_45_pool_full() {
        let mut pool = super::Xc45Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_45_pool_drain() {
        let mut pool = super::Xc45Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_45_pool_stats() {
        let mut pool = super::Xc45Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_45_pool_clear() {
        let mut pool = super::Xc45Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_45_pool_shrink() {
        let mut pool = super::Xc45Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_45_pool_default() {
        let pool: super::Xc45Pool<String> = super::Xc45Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_45_pool_extend() {
        let mut pool = super::Xc45Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_45_pool_retain() {
        let mut pool = super::Xc45Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_45_scheduler_round_robin() {
        let mut sched = super::Xc45Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_45_scheduler_empty() {
        let mut sched = super::Xc45Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_45_scheduler_reset() {
        let mut sched = super::Xc45Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_45_scheduler_add_remove() {
        let mut sched = super::Xc45Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_45_scheduler_targets() {
        let sched = super::Xc45Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_45_hash_empty() {
        assert_eq!(super::xc_45_hash(b""), 5381);
    }

    #[test]
    fn xc_45_hash_data() {
        let h = super::xc_45_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_45_hash(b"hello"), h);
    }

    #[test]
    fn xc_45_reverse_str() {
        assert_eq!(super::xc_45_reverse("abc"), "cba");
        assert_eq!(super::xc_45_reverse(""), "");
    }


    #[test]
    fn xe_87_pipeline_empty() {
        let p = super::Xe87Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_87_pipeline_parse_stage() {
        let p = super::Xe87Pipeline::new()
            .add_parse(super::xe_87_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_87_pipeline_transform_double() {
        let p = super::Xe87Pipeline::new()
            .add_transform(super::xe_87_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_87_pipeline_validate_reverse() {
        let p = super::Xe87Pipeline::new()
            .add_validate(super::xe_87_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_87_pipeline_emit_filter() {
        let p = super::Xe87Pipeline::new()
            .add_emit(super::xe_87_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_87_pipeline_multi_stage() {
        let p = super::Xe87Pipeline::new()
            .add_parse(super::xe_87_pipeline_identity)
            .add_transform(super::xe_87_pipeline_double)
            .add_validate(super::xe_87_pipeline_reverse)
            .add_emit(super::xe_87_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_87_pipeline_error_propagation() {
        let p = super::Xe87Pipeline::new()
            .add_parse(super::xe_87_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe87Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_87_pipeline_compose() {
        let p1 = super::Xe87Pipeline::new()
            .add_parse(super::xe_87_pipeline_identity);
        let p2 = super::Xe87Pipeline::new()
            .add_transform(super::xe_87_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_87_pipeline_error_display() {
        let e = super::Xe87PipelineError {
            stage: super::Xe87Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_87_cache_put_get() {
        let mut c = super::Xe87Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_87_cache_miss() {
        let mut c: super::Xe87Cache<&str, i32> = super::Xe87Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_87_cache_ttl_expiry() {
        let mut c = super::Xe87Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_87_cache_evict() {
        let mut c = super::Xe87Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_87_cache_capacity() {
        let mut c = super::Xe87Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_87_cache_stats() {
        let mut c = super::Xe87Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_87_cache_clear() {
        let mut c = super::Xe87Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_85 graph tests ------------------------------------------------

    #[test]
    fn xg_85_graph_empty() {
        let g = super::Xg85Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_85_graph_add_node() {
        let mut g = super::Xg85Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_85_graph_add_edge() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_85_graph_neighbors() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_85_graph_has_path() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_85_graph_self_path() {
        let g = super::Xg85Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_85_graph_topo_sort() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_85_graph_cycle_detect_false() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_85_graph_cycle_detect_true() {
        let mut g = super::Xg85Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_85 heap tests -------------------------------------------------

    #[test]
    fn xg_85_heap_empty() {
        let h: super::Xg85Heap<i32> = super::Xg85Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_85_heap_push_pop() {
        let mut h = super::Xg85Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_85_heap_peek() {
        let mut h = super::Xg85Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_85_heap_drain_sorted() {
        let mut h = super::Xg85Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_85_heap_merge() {
        let mut a = super::Xg85Heap::new();
        let mut b = super::Xg85Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_85_heap_default() {
        let h: super::Xg85Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_85_graph_default() {
        let g: super::Xg85Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh44_skip_insert_contains() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh44_skip_remove() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh44_skip_len() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh44_skip_range_query() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh44_skip_floor_ceiling() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh44_skip_rank() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh44_skip_empty() {
        let sl = super::Xh44SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh44_skip_duplicates() {
        let mut sl = super::Xh44SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh44_bitset_set_test() {
        let mut bs = super::Xh44BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh44_bitset_clear_count() {
        let mut bs = super::Xh44BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh44_bitset_and_or_xor() {
        let mut a = super::Xh44BitSet::xh_new(128);
        let mut b = super::Xh44BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh44_bitset_iter_ones() {
        let mut bs = super::Xh44BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh44_bitset_first_last() {
        let mut bs = super::Xh44BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh44_bitset_empty() {
        let bs = super::Xh44BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi44_deque_push_pop_back() {
        let mut dq = super::Xi44Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi44_deque_push_pop_front() {
        let mut dq = super::Xi44Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi44_deque_mixed_ops() {
        let mut dq = super::Xi44Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi44_deque_get_and_split() {
        let mut dq = super::Xi44Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi44_deque_rotate_left() {
        let mut dq = super::Xi44Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi44_deque_rotate_right() {
        let mut dq = super::Xi44Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi44_deque_grow() {
        let mut dq = super::Xi44Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi44_deque_empty() {
        let dq = super::Xi44Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi44_interval_tree_insert_query() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi44Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi44Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi44_interval_tree_overlap() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi44Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi44Interval::xi_new(12, 20));
        let q = super::Xi44Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi44_interval_tree_remove() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi44Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi44_interval_tree_gaps() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi44Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi44Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi44Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi44Interval::xi_new(8, 10));
    }

    #[test]
    fn xi44_interval_tree_merge() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi44Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi44Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi44Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi44Interval::xi_new(10, 15));
    }

    #[test]
    fn xi44_interval_tree_all() {
        let mut tree = super::Xi44IntervalTree::xi_new();
        tree.xi_insert(super::Xi44Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi44Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi44_interval_tree_empty() {
        let tree = super::Xi44IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi44_interval_tree_contains_point() {
        let iv = super::Xi44Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 44) ---

    #[test]
    fn xj_44_uf_make_and_find() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_44_uf_union_connected() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_44_uf_component_count() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_44_uf_component_size() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_44_uf_largest_component() {
        let mut uf = super::Xj44UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_44_uf_many_elements() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_44_uf_separate_components() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_44_uf_path_compression() {
        let mut uf = super::Xj44UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_44_bt_insert_get() {
        let mut bt = super::Xj44BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_44_bt_contains_len() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_44_bt_replace() {
        let mut bt = super::Xj44BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_44_bt_remove() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_44_bt_keys_values() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_44_bt_range() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_44_bt_min_max() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_44_bt_many_inserts() {
        let mut bt = super::Xj44BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_44 segment tree tests ---

    #[test]
    fn xk_44_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_44_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk44SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_44_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_44_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_44_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_44_st_single_element() {
        let data = vec![42];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_44_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk44SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_44_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk44SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_44 disjoint intervals tests ---

    #[test]
    fn xk_44_di_add_and_count() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_44_di_merge_overlap() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_44_di_contains() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_44_di_remove() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_44_di_covered_length() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_44_di_gaps() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_44_di_merge_adjacent() {
        let mut di = super::Xk44DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_44_di_empty() {
        let di = super::Xk44DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_44_rope_new_empty() {
        let rope = super::Xl44Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_44_rope_from_str() {
        let rope = super::Xl44Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_44_rope_insert_at() {
        let mut rope = super::Xl44Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_44_rope_delete_range() {
        let mut rope = super::Xl44Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_44_rope_char_at() {
        let rope = super::Xl44Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_44_rope_split_concat() {
        let rope = super::Xl44Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_44_rope_line_count() {
        let rope = super::Xl44Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_44_rope_line_at() {
        let rope = super::Xl44Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_44_sa_build_and_search() {
        let sa = super::Xl44SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_44_sa_count() {
        let sa = super::Xl44SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_44_sa_longest_repeated() {
        let sa = super::Xl44SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_44_sa_all_positions() {
        let sa = super::Xl44SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_44_sa_len() {
        let sa = super::Xl44SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_44_sa_empty() {
        let sa = super::Xl44SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_44_rope_slice() {
        let rope = super::Xl44Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_44_sa_search_start() {
        let sa = super::Xl44SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_44_sparse_set_get() {
        let mut m = super::Xm44MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_44_sparse_row_col() {
        let mut m = super::Xm44MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_44_sparse_transpose() {
        let mut m = super::Xm44MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_44_sparse_multiply_vec() {
        let mut m = super::Xm44MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_44_sparse_nnz_density() {
        let mut m = super::Xm44MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_44_sparse_clear() {
        let mut m = super::Xm44MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_44_sparse_overwrite_zero() {
        let mut m = super::Xm44MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_44_tokenizer_basic() {
        let t = super::Xm44Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_44_tokenizer_count() {
        let t = super::Xm44Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_44_tokenizer_unique() {
        let t = super::Xm44Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_44_tokenizer_frequency() {
        let t = super::Xm44Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_44_tokenizer_delimiter() {
        let t = super::Xm44Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_44_tokenizer_whitespace() {
        let t = super::Xm44Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_44_tokenizer_empty() {
        let t = super::Xm44Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}
