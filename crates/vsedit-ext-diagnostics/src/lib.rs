//! Ext API: Diagnostics.
//!
//! RPC bridge between the extension host and the main thread for diagnostics.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_diagnostics";

// ── RPC message types ──

/// Messages exchanged for the `Diagnostics` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiagnosticMessage {
    SetDiagnostics { collection: String, uri: String, diagnostics: Vec<Diagnostic> },
    ClearDiagnostics { collection: String, uri: Option<String> },
    GetDiagnostics { uri: Option<String> },
}

/// A single diagnostic (error, warning, etc.) within a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub message: String,
    pub severity: DiagnosticSeverity,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub related_info: Vec<DiagnosticRelatedInfo>,
    #[serde(default)]
    pub tags: Vec<DiagnosticTag>,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Additional location and message related to a diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRelatedInfo {
    pub uri: String,
    pub start_line: u32,
    pub start_col: u32,
    pub message: String,
}

/// Tags that modify diagnostic rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticTag {
    Unnecessary,
    Deprecated,
}

/// A named collection of diagnostics keyed by document URI.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticCollection {
    pub name: String,
    pub entries: HashMap<String, Vec<Diagnostic>>,
}

/// Response payload for diagnostic operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiagnosticResponse {
    Ok,
    Diagnostics { entries: Vec<(String, Vec<Diagnostic>)> },
}

// ── Bridge ──

/// Manages diagnostic collections published by extensions.
#[derive(Debug, Default)]
pub struct DiagnosticBridge {
    collections: HashMap<String, DiagnosticCollection>,
}

impl DiagnosticBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming diagnostic message and return a response.
    pub fn handle(&mut self, msg: DiagnosticMessage) -> DiagnosticResponse {
        match msg {
            DiagnosticMessage::SetDiagnostics { collection, uri, diagnostics } => {
                let col = self.collections.entry(collection.clone()).or_insert_with(|| {
                    DiagnosticCollection { name: collection, entries: HashMap::new() }
                });
                col.entries.insert(uri, diagnostics);
                DiagnosticResponse::Ok
            }
            DiagnosticMessage::ClearDiagnostics { collection, uri } => {
                if let Some(col) = self.collections.get_mut(&collection) {
                    if let Some(u) = uri {
                        col.entries.remove(&u);
                    } else {
                        col.entries.clear();
                    }
                }
                DiagnosticResponse::Ok
            }
            DiagnosticMessage::GetDiagnostics { uri } => {
                let mut entries = Vec::new();
                for col in self.collections.values() {
                    for (u, diags) in &col.entries {
                        if uri.as_ref().is_none_or(|filter| filter == u) {
                            entries.push((u.clone(), diags.clone()));
                        }
                    }
                }
                DiagnosticResponse::Diagnostics { entries }
            }
        }
    }

    pub fn collection_count(&self) -> usize {
        self.collections.len()
    }

    /// Total number of diagnostics across all collections.
    pub fn total_diagnostics(&self) -> usize {
        self.collections.values().map(|c| c.entries.values().map(Vec::len).sum::<usize>()).sum()
    }
}

// ── Error types ──

/// Errors that can occur during diagnostic operations.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticError {
    /// The diagnostic span is invalid (end before start).
    InvalidSpan { start_line: u32, start_col: u32, end_line: u32, end_col: u32 },
    /// The collection name is empty.
    EmptyCollectionName,
    /// The URI is empty or invalid.
    InvalidUri(String),
    /// The diagnostic message is empty.
    EmptyMessage,
}

impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticError::InvalidSpan { start_line, start_col, end_line, end_col } => {
                write!(
                    f,
                    "invalid span: ({}, {}) to ({}, {})",
                    start_line, start_col, end_line, end_col
                )
            }
            DiagnosticError::EmptyCollectionName => write!(f, "collection name must not be empty"),
            DiagnosticError::InvalidUri(uri) => write!(f, "invalid URI: '{}'", uri),
            DiagnosticError::EmptyMessage => write!(f, "diagnostic message must not be empty"),
        }
    }
}

impl std::error::Error for DiagnosticError {}

// ── Display impls ──

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverity::Error => write!(f, "error"),
            DiagnosticSeverity::Warning => write!(f, "warning"),
            DiagnosticSeverity::Information => write!(f, "info"),
            DiagnosticSeverity::Hint => write!(f, "hint"),
        }
    }
}

impl fmt::Display for DiagnosticTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticTag::Unnecessary => write!(f, "unnecessary"),
            DiagnosticTag::Deprecated => write!(f, "deprecated"),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}:{}-{}:{}: {}",
            self.severity, self.start_line, self.start_col, self.end_line, self.end_col, self.message
        )
    }
}

// ── Diagnostic severity helpers ──

impl DiagnosticSeverity {
    /// Returns a numeric weight for ordering (lower is more severe).
    pub fn weight(self) -> u8 {
        match self {
            DiagnosticSeverity::Error => 0,
            DiagnosticSeverity::Warning => 1,
            DiagnosticSeverity::Information => 2,
            DiagnosticSeverity::Hint => 3,
        }
    }

    /// Returns `true` if this severity blocks a build (error only).
    pub fn is_blocking(self) -> bool {
        matches!(self, DiagnosticSeverity::Error)
    }
}

impl PartialOrd for DiagnosticSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.weight().cmp(&other.weight()))
    }
}

impl Ord for DiagnosticSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.weight().cmp(&other.weight())
    }
}

// ── Diagnostic validation ──

impl Diagnostic {
    /// Validate that this diagnostic has a well-formed span and non-empty message.
    pub fn validate(&self) -> Result<(), DiagnosticError> {
        if self.message.is_empty() {
            return Err(DiagnosticError::EmptyMessage);
        }
        if self.end_line < self.start_line
            || (self.end_line == self.start_line && self.end_col < self.start_col)
        {
            return Err(DiagnosticError::InvalidSpan {
                start_line: self.start_line,
                start_col: self.start_col,
                end_line: self.end_line,
                end_col: self.end_col,
            });
        }
        Ok(())
    }

    /// Returns `true` if this diagnostic spans multiple lines.
    pub fn is_multiline(&self) -> bool {
        self.end_line > self.start_line
    }

    /// Returns the span length in columns (only meaningful for single-line diagnostics).
    pub fn span_length(&self) -> Option<u32> {
        if self.is_multiline() {
            None
        } else {
            Some(self.end_col.saturating_sub(self.start_col))
        }
    }

    /// Returns `true` if the diagnostic has any tags.
    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }
}

// ── Diagnostic builder ──

/// Builder for constructing `Diagnostic` instances.
#[derive(Debug, Clone)]
pub struct DiagnosticBuilder {
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    message: String,
    severity: DiagnosticSeverity,
    code: Option<String>,
    source: Option<String>,
    related_info: Vec<DiagnosticRelatedInfo>,
    tags: Vec<DiagnosticTag>,
}

impl DiagnosticBuilder {
    pub fn new(message: impl Into<String>, severity: DiagnosticSeverity) -> Self {
        Self {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
            message: message.into(),
            severity,
            code: None,
            source: None,
            related_info: Vec::new(),
            tags: Vec::new(),
        }
    }

    pub fn span(mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        self.start_line = start_line;
        self.start_col = start_col;
        self.end_line = end_line;
        self.end_col = end_col;
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn tag(mut self, tag: DiagnosticTag) -> Self {
        self.tags.push(tag);
        self
    }

    pub fn related(mut self, info: DiagnosticRelatedInfo) -> Self {
        self.related_info.push(info);
        self
    }

    /// Build and validate the diagnostic.
    pub fn build(self) -> Result<Diagnostic, DiagnosticError> {
        let diag = Diagnostic {
            start_line: self.start_line,
            start_col: self.start_col,
            end_line: self.end_line,
            end_col: self.end_col,
            message: self.message,
            severity: self.severity,
            code: self.code,
            source: self.source,
            related_info: self.related_info,
            tags: self.tags,
        };
        diag.validate()?;
        Ok(diag)
    }
}

// ── DiagnosticCollection helpers ──

impl PartialEq for DiagnosticCollection {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.entries == other.entries
    }
}

impl DiagnosticCollection {
    /// Create a new named collection.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), entries: HashMap::new() }
    }

    /// Total number of diagnostics in this collection.
    pub fn diagnostic_count(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }

    /// Number of distinct URIs with diagnostics.
    pub fn uri_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns all diagnostics with the given severity across all URIs.
    pub fn filter_by_severity(&self, severity: DiagnosticSeverity) -> Vec<(&str, &Diagnostic)> {
        self.entries
            .iter()
            .flat_map(|(uri, diags)| {
                diags.iter().filter(move |d| d.severity == severity).map(move |d| (uri.as_str(), d))
            })
            .collect()
    }

    /// Returns the most severe diagnostic level present, if any.
    pub fn worst_severity(&self) -> Option<DiagnosticSeverity> {
        self.entries
            .values()
            .flat_map(|diags| diags.iter().map(|d| d.severity))
            .min()
    }

    /// Returns `true` if any diagnostic in this collection is an error.
    pub fn has_errors(&self) -> bool {
        self.worst_severity() == Some(DiagnosticSeverity::Error)
    }
}

// ── DiagnosticBridge extensions ──

impl DiagnosticBridge {
    /// Validate and then process a message; returns an error if the message
    /// contains invalid data.
    pub fn handle_validated(
        &mut self,
        msg: DiagnosticMessage,
    ) -> Result<DiagnosticResponse, DiagnosticError> {
        match &msg {
            DiagnosticMessage::SetDiagnostics { collection, uri, diagnostics } => {
                if collection.is_empty() {
                    return Err(DiagnosticError::EmptyCollectionName);
                }
                if uri.is_empty() {
                    return Err(DiagnosticError::InvalidUri(uri.clone()));
                }
                for d in diagnostics {
                    d.validate()?;
                }
            }
            DiagnosticMessage::ClearDiagnostics { collection, .. } => {
                if collection.is_empty() {
                    return Err(DiagnosticError::EmptyCollectionName);
                }
            }
            DiagnosticMessage::GetDiagnostics { .. } => {}
        }
        Ok(self.handle(msg))
    }

    /// Returns all collection names.
    pub fn collection_names(&self) -> Vec<&str> {
        self.collections.keys().map(String::as_str).collect()
    }

    /// Returns `true` if any collection contains errors.
    pub fn has_errors(&self) -> bool {
        self.collections.values().any(|c| c.has_errors())
    }

    /// Count diagnostics of a specific severity across all collections.
    pub fn count_by_severity(&self, severity: DiagnosticSeverity) -> usize {
        self.collections
            .values()
            .map(|c| c.filter_by_severity(severity).len())
            .sum()
    }
}

/// Initialize the diagnostics extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ---------------------------------------------------------------------------
// Diagnostic severity aggregation
// ---------------------------------------------------------------------------

/// Aggregated counts of diagnostics by severity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeverityAggregation {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

impl SeverityAggregation {
    /// Build an aggregation from a slice of diagnostics.
    pub fn from_diagnostics(diags: &[Diagnostic]) -> Self {
        let mut agg = Self::default();
        for d in diags {
            match d.severity {
                DiagnosticSeverity::Error => agg.errors += 1,
                DiagnosticSeverity::Warning => agg.warnings += 1,
                DiagnosticSeverity::Information => agg.infos += 1,
                DiagnosticSeverity::Hint => agg.hints += 1,
            }
        }
        agg
    }

    /// Total number of diagnostics.
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.infos + self.hints
    }

    /// Returns `true` if there are no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

impl fmt::Display for SeverityAggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E:{} W:{} I:{} H:{}", self.errors, self.warnings, self.infos, self.hints)
    }
}

// ---------------------------------------------------------------------------
// Diagnostic deduplication
// ---------------------------------------------------------------------------

/// Remove duplicate diagnostics from a list, keeping the first occurrence.
/// Two diagnostics are considered duplicates if they have the same span,
/// message, and severity.
pub fn deduplicate_diagnostics(diags: &[Diagnostic]) -> Vec<Diagnostic> {
    let mut seen = Vec::new();
    let mut result = Vec::new();
    for d in diags {
        let key = (d.start_line, d.start_col, d.end_line, d.end_col, &d.message, d.severity);
        if !seen.contains(&key) {
            seen.push(key);
            result.push(d.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Diagnostic range intersection
// ---------------------------------------------------------------------------

/// Check whether two diagnostic spans intersect.
pub fn ranges_intersect(a: &Diagnostic, b: &Diagnostic) -> bool {
    // No intersection if one ends before the other starts
    if a.end_line < b.start_line || b.end_line < a.start_line {
        return false;
    }
    if a.end_line == b.start_line && a.end_col <= b.start_col {
        return false;
    }
    if b.end_line == a.start_line && b.end_col <= a.start_col {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Quick fix suggestion tracking
// ---------------------------------------------------------------------------

/// A quick fix suggestion associated with a diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuickFixSuggestion {
    pub title: String,
    pub replacement_text: String,
    pub diagnostic_index: usize,
}

// ---------------------------------------------------------------------------
// DiagnosticSeverityCounter — tallies errors/warnings/info/hints
// ---------------------------------------------------------------------------

/// Incremental counter for diagnostic severities.
///
/// Unlike [`SeverityAggregation`] which is built from a slice, this counter
/// can be incrementally updated as diagnostics are added or removed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticSeverityCounter {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

impl DiagnosticSeverityCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the count for the given severity.
    pub fn increment(&mut self, severity: DiagnosticSeverity) {
        match severity {
            DiagnosticSeverity::Error => self.errors += 1,
            DiagnosticSeverity::Warning => self.warnings += 1,
            DiagnosticSeverity::Information => self.infos += 1,
            DiagnosticSeverity::Hint => self.hints += 1,
        }
    }

    /// Decrement the count for the given severity (saturating).
    pub fn decrement(&mut self, severity: DiagnosticSeverity) {
        match severity {
            DiagnosticSeverity::Error => self.errors = self.errors.saturating_sub(1),
            DiagnosticSeverity::Warning => self.warnings = self.warnings.saturating_sub(1),
            DiagnosticSeverity::Information => self.infos = self.infos.saturating_sub(1),
            DiagnosticSeverity::Hint => self.hints = self.hints.saturating_sub(1),
        }
    }

    /// Get the count for a specific severity.
    pub fn get(&self, severity: DiagnosticSeverity) -> usize {
        match severity {
            DiagnosticSeverity::Error => self.errors,
            DiagnosticSeverity::Warning => self.warnings,
            DiagnosticSeverity::Information => self.infos,
            DiagnosticSeverity::Hint => self.hints,
        }
    }

    /// Total count across all severities.
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.infos + self.hints
    }

    /// Reset all counts to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Returns the most severe level that has a non-zero count.
    pub fn worst_severity(&self) -> Option<DiagnosticSeverity> {
        if self.errors > 0 {
            Some(DiagnosticSeverity::Error)
        } else if self.warnings > 0 {
            Some(DiagnosticSeverity::Warning)
        } else if self.infos > 0 {
            Some(DiagnosticSeverity::Information)
        } else if self.hints > 0 {
            Some(DiagnosticSeverity::Hint)
        } else {
            None
        }
    }
}

impl fmt::Display for DiagnosticSeverityCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "errors={}, warnings={}, info={}, hints={}",
            self.errors, self.warnings, self.infos, self.hints
        )
    }
}

// ---------------------------------------------------------------------------
// filter_diagnostics — filter by severity and/or source
// ---------------------------------------------------------------------------

/// Filter diagnostics by severity, source, or both.
///
/// If `severity` is `Some`, only diagnostics matching that severity are included.
/// If `source` is `Some`, only diagnostics whose source matches (case-insensitive) are included.
pub fn filter_diagnostics(
    diags: &[Diagnostic],
    severity: Option<DiagnosticSeverity>,
    source: Option<&str>,
) -> Vec<Diagnostic> {
    diags
        .iter()
        .filter(|d| {
            if let Some(sev) = severity {
                if d.severity != sev {
                    return false;
                }
            }
            if let Some(src) = source {
                match &d.source {
                    Some(ds) => {
                        if !ds.eq_ignore_ascii_case(src) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// diagnostic_sort — sort by severity then line
// ---------------------------------------------------------------------------

/// Sort diagnostics by severity (most severe first), then by start line,
/// then by start column.
pub fn diagnostic_sort(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        a.severity
            .weight()
            .cmp(&b.severity.weight())
            .then(a.start_line.cmp(&b.start_line))
            .then(a.start_col.cmp(&b.start_col))
    });
}

/// Tracks quick fix suggestions for diagnostics.
#[derive(Debug, Default)]
pub struct QuickFixTracker {
    suggestions: Vec<QuickFixSuggestion>,
}

impl QuickFixTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, suggestion: QuickFixSuggestion) {
        self.suggestions.push(suggestion);
    }

    pub fn suggestions_for(&self, diagnostic_index: usize) -> Vec<&QuickFixSuggestion> {
        self.suggestions.iter().filter(|s| s.diagnostic_index == diagnostic_index).collect()
    }

    pub fn total_suggestions(&self) -> usize {
        self.suggestions.len()
    }

    pub fn clear(&mut self) {
        self.suggestions.clear();
    }
}

// ---------------------------------------------------------------------------
// Additional DiagnosticCollection methods
// ---------------------------------------------------------------------------

impl DiagnosticCollection {
    /// Remove all diagnostics from this collection.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns references to all diagnostics across all URIs.
    pub fn all_diagnostics(&self) -> Vec<&Diagnostic> {
        self.entries.values().flat_map(|v| v.iter()).collect()
    }
}

// ---------------------------------------------------------------------------
// Additional DiagnosticBridge methods
// ---------------------------------------------------------------------------

impl DiagnosticBridge {
    /// Returns a human-readable summary of all collections.
    pub fn summary(&self) -> String {
        let total: usize = self.collections.values().map(|c| c.diagnostic_count()).sum();
        let cols = self.collections.len();
        format!("{} diagnostic(s) in {} collection(s)", total, cols)
    }
}

// ---------------------------------------------------------------------------
// Display for DiagnosticBridge
// ---------------------------------------------------------------------------

impl fmt::Display for DiagnosticBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// Additional Diagnostic methods
// ---------------------------------------------------------------------------

impl Diagnostic {
    /// Returns a human-readable label for the diagnostic severity.
    pub fn severity_label(&self) -> &str {
        match self.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }

    /// Returns `true` if this diagnostic is a warning.
    pub fn is_warning(&self) -> bool {
        self.severity == DiagnosticSeverity::Warning
    }

    /// Returns `true` if this diagnostic is a hint.
    pub fn is_hint(&self) -> bool {
        self.severity == DiagnosticSeverity::Hint
    }
}

// ---------------------------------------------------------------------------
// Additional SeverityAggregation methods
// ---------------------------------------------------------------------------

impl SeverityAggregation {
    /// Returns the most severe diagnostic level present, if any.
    pub fn worst(&self) -> Option<DiagnosticSeverity> {
        if self.errors > 0 {
            Some(DiagnosticSeverity::Error)
        } else if self.warnings > 0 {
            Some(DiagnosticSeverity::Warning)
        } else if self.infos > 0 {
            Some(DiagnosticSeverity::Information)
        } else if self.hints > 0 {
            Some(DiagnosticSeverity::Hint)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticFilter — builder-pattern filter for diagnostics
// ---------------------------------------------------------------------------

/// A composable filter for selecting diagnostics by severity range, source,
/// file pattern, and tag presence.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticFilter {
    min_severity: Option<DiagnosticSeverity>,
    max_severity: Option<DiagnosticSeverity>,
    sources: Vec<String>,
    file_patterns: Vec<String>,
    required_tags: Vec<DiagnosticTag>,
    excluded_tags: Vec<DiagnosticTag>,
}

impl DiagnosticFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Only include diagnostics at least as severe as `sev` (lower weight = more severe).
    pub fn min_severity(mut self, sev: DiagnosticSeverity) -> Self {
        self.min_severity = Some(sev);
        self
    }

    /// Only include diagnostics at most as severe as `sev`.
    pub fn max_severity(mut self, sev: DiagnosticSeverity) -> Self {
        self.max_severity = Some(sev);
        self
    }

    /// Only include diagnostics from one of the listed sources (case-insensitive).
    pub fn source(mut self, src: impl Into<String>) -> Self {
        self.sources.push(src.into());
        self
    }

    /// Only include diagnostics whose URI contains the given substring.
    pub fn file_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.file_patterns.push(pattern.into());
        self
    }

    /// Only include diagnostics that carry the given tag.
    pub fn require_tag(mut self, tag: DiagnosticTag) -> Self {
        self.required_tags.push(tag);
        self
    }

    /// Exclude diagnostics that carry the given tag.
    pub fn exclude_tag(mut self, tag: DiagnosticTag) -> Self {
        self.excluded_tags.push(tag);
        self
    }

    /// Returns `true` if the diagnostic passes all configured predicates.
    pub fn matches(&self, diag: &Diagnostic, uri: Option<&str>) -> bool {
        if let Some(min) = self.min_severity {
            if diag.severity.weight() > min.weight() {
                return false;
            }
        }
        if let Some(max) = self.max_severity {
            if diag.severity.weight() < max.weight() {
                return false;
            }
        }
        if !self.sources.is_empty() {
            let ok = match &diag.source {
                Some(ds) => self.sources.iter().any(|s| s.eq_ignore_ascii_case(ds)),
                None => false,
            };
            if !ok {
                return false;
            }
        }
        if let Some(u) = uri {
            if !self.file_patterns.is_empty()
                && !self.file_patterns.iter().any(|p| u.contains(p.as_str()))
            {
                return false;
            }
        }
        for tag in &self.required_tags {
            if !diag.tags.contains(tag) {
                return false;
            }
        }
        for tag in &self.excluded_tags {
            if diag.tags.contains(tag) {
                return false;
            }
        }
        true
    }

    /// Apply the filter to a slice of diagnostics (without URI context).
    pub fn apply<'a>(&self, diags: &'a [Diagnostic]) -> Vec<&'a Diagnostic> {
        diags.iter().filter(|d| self.matches(d, None)).collect()
    }

    /// Apply the filter to a `DiagnosticCollection`, returning matching
    /// `(uri, diagnostic)` pairs.
    pub fn apply_to_collection<'a>(
        &self,
        col: &'a DiagnosticCollection,
    ) -> Vec<(&'a str, &'a Diagnostic)> {
        col.entries
            .iter()
            .flat_map(|(uri, diags)| {
                diags
                    .iter()
                    .filter(|d| self.matches(d, Some(uri)))
                    .map(move |d| (uri.as_str(), d))
            })
            .collect()
    }
}

impl fmt::Display for DiagnosticFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(min) = self.min_severity {
            parts.push(format!("min_severity={}", min));
        }
        if let Some(max) = self.max_severity {
            parts.push(format!("max_severity={}", max));
        }
        if !self.sources.is_empty() {
            parts.push(format!("sources={:?}", self.sources));
        }
        if !self.file_patterns.is_empty() {
            parts.push(format!("files={:?}", self.file_patterns));
        }
        if parts.is_empty() {
            write!(f, "DiagnosticFilter(all)")
        } else {
            write!(f, "DiagnosticFilter({})", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticDiff — compare two diagnostic sets
// ---------------------------------------------------------------------------

/// The result of comparing two sets of diagnostics for a single URI.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticDiff {
    pub added: Vec<Diagnostic>,
    pub removed: Vec<Diagnostic>,
    pub unchanged: Vec<Diagnostic>,
}

impl DiagnosticDiff {
    /// Compare `before` and `after` diagnostic slices for the same document.
    ///
    /// A diagnostic is considered the same if it has the same span, message,
    /// and severity.
    pub fn compute(before: &[Diagnostic], after: &[Diagnostic]) -> Self {
        let key = |d: &Diagnostic| {
            (d.start_line, d.start_col, d.end_line, d.end_col, d.message.clone(), d.severity)
        };
        let before_keys: Vec<_> = before.iter().map(|d| key(d)).collect();
        let after_keys: Vec<_> = after.iter().map(|d| key(d)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut unchanged = Vec::new();

        // Track which `after` entries were matched
        let mut matched_after = vec![false; after.len()];

        for (i, bk) in before_keys.iter().enumerate() {
            if let Some(j) = after_keys.iter().enumerate().position(|(j, ak)| !matched_after[j] && ak == bk) {
                matched_after[j] = true;
                unchanged.push(before[i].clone());
            } else {
                removed.push(before[i].clone());
            }
        }

        for (j, _) in after.iter().enumerate() {
            if !matched_after[j] {
                added.push(after[j].clone());
            }
        }

        Self { added, removed, unchanged }
    }

    /// Compare two entire collections, producing per-URI diffs.
    pub fn compute_collections(
        before: &DiagnosticCollection,
        after: &DiagnosticCollection,
    ) -> HashMap<String, DiagnosticDiff> {
        let mut result = HashMap::new();
        let empty = Vec::new();

        let mut all_uris: Vec<&String> =
            before.entries.keys().chain(after.entries.keys()).collect();
        all_uris.sort();
        all_uris.dedup();

        for uri in all_uris {
            let b = before.entries.get(uri).unwrap_or(&empty);
            let a = after.entries.get(uri).unwrap_or(&empty);
            let diff = Self::compute(b, a);
            if !diff.added.is_empty() || !diff.removed.is_empty() {
                result.insert(uri.clone(), diff);
            }
        }
        result
    }

    /// `true` if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Total number of changes (added + removed).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

impl fmt::Display for DiagnosticDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "+{} -{} ={} diagnostic(s)",
            self.added.len(),
            self.removed.len(),
            self.unchanged.len()
        )
    }
}

// ---------------------------------------------------------------------------
// DiagnosticGrouping — group diagnostics by file, severity, or source
// ---------------------------------------------------------------------------

/// Diagnostics grouped by a chosen dimension with summary helpers.
#[derive(Debug, Clone)]
pub struct DiagnosticGrouping {
    pub groups: HashMap<String, Vec<Diagnostic>>,
}

impl DiagnosticGrouping {
    /// Group a collection's diagnostics by URI (file).
    pub fn by_file(col: &DiagnosticCollection) -> Self {
        Self { groups: col.entries.clone() }
    }

    /// Group diagnostics by severity label.
    pub fn by_severity(diags: &[Diagnostic]) -> Self {
        let mut groups: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for d in diags {
            groups.entry(d.severity.to_string()).or_default().push(d.clone());
        }
        Self { groups }
    }

    /// Group diagnostics by source (diagnostics without a source go under `"<unknown>"`).
    pub fn by_source(diags: &[Diagnostic]) -> Self {
        let mut groups: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for d in diags {
            let key = d.source.clone().unwrap_or_else(|| "<unknown>".to_string());
            groups.entry(key).or_default().push(d.clone());
        }
        Self { groups }
    }

    /// Number of distinct groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total diagnostic count across all groups.
    pub fn total(&self) -> usize {
        self.groups.values().map(Vec::len).sum()
    }

    /// Return a sorted summary: `(group_key, count)` pairs ordered by count descending.
    pub fn summary(&self) -> Vec<(String, usize)> {
        let mut v: Vec<_> = self.groups.iter().map(|(k, d)| (k.clone(), d.len())).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Return the group with the most diagnostics, if any.
    pub fn largest_group(&self) -> Option<(&str, usize)> {
        self.groups
            .iter()
            .max_by_key(|(_, v)| v.len())
            .map(|(k, v)| (k.as_str(), v.len()))
    }
}

impl fmt::Display for DiagnosticGrouping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let summary = self.summary();
        for (key, count) in &summary {
            writeln!(f, "{}: {}", key, count)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// merge_collections — merge two DiagnosticCollections
// ---------------------------------------------------------------------------

/// Merge two `DiagnosticCollection`s into a new one.
///
/// The resulting collection uses `merged_name` as its name.  For URIs that
/// appear in both collections the diagnostic lists are concatenated (duplicates
/// are *not* removed — use [`deduplicate_diagnostics`] afterwards if needed).
pub fn merge_collections(
    a: &DiagnosticCollection,
    b: &DiagnosticCollection,
    merged_name: impl Into<String>,
) -> DiagnosticCollection {
    let mut merged = DiagnosticCollection::new(merged_name);
    for (uri, diags) in &a.entries {
        merged.entries.entry(uri.clone()).or_default().extend(diags.iter().cloned());
    }
    for (uri, diags) in &b.entries {
        merged.entries.entry(uri.clone()).or_default().extend(diags.iter().cloned());
    }
    merged
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<DiagnosticSeverityCounter> for SeverityAggregation {
    fn from(c: DiagnosticSeverityCounter) -> Self {
        Self { errors: c.errors, warnings: c.warnings, infos: c.infos, hints: c.hints }
    }
}

impl From<SeverityAggregation> for DiagnosticSeverityCounter {
    fn from(a: SeverityAggregation) -> Self {
        Self { errors: a.errors, warnings: a.warnings, infos: a.infos, hints: a.hints }
    }
}

// -- DiagnosticSeverityFilter ------------------------------------------------

/// Filter diagnostics by severity levels.
#[derive(Debug, Clone)]
pub struct DiagnosticSeverityFilter {
    pub show_errors: bool,
    pub show_warnings: bool,
    pub show_info: bool,
    pub show_hints: bool,
}

impl Default for DiagnosticSeverityFilter {
    fn default() -> Self {
        Self {
            show_errors: true,
            show_warnings: true,
            show_info: true,
            show_hints: true,
        }
    }
}

impl DiagnosticSeverityFilter {
    /// Create a filter that shows all severities.
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a filter that shows only errors.
    pub fn errors_only() -> Self {
        Self {
            show_errors: true,
            show_warnings: false,
            show_info: false,
            show_hints: false,
        }
    }

    /// Check if a severity passes this filter.
    pub fn matches(&self, severity: &DiagnosticSeverity) -> bool {
        match severity {
            DiagnosticSeverity::Error => self.show_errors,
            DiagnosticSeverity::Warning => self.show_warnings,
            DiagnosticSeverity::Information => self.show_info,
            DiagnosticSeverity::Hint => self.show_hints,
        }
    }

    /// Filter a list of diagnostics, returning only those matching.
    pub fn apply<'a>(&self, diagnostics: &'a [Diagnostic]) -> Vec<&'a Diagnostic> {
        diagnostics.iter().filter(|d| self.matches(&d.severity)).collect()
    }

    /// Count of enabled severity levels.
    pub fn enabled_count(&self) -> usize {
        [self.show_errors, self.show_warnings, self.show_info, self.show_hints]
            .iter()
            .filter(|&&v| v)
            .count()
    }
}

impl fmt::Display for DiagnosticSeverityFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut enabled = Vec::new();
        if self.show_errors { enabled.push("errors"); }
        if self.show_warnings { enabled.push("warnings"); }
        if self.show_info { enabled.push("info"); }
        if self.show_hints { enabled.push("hints"); }
        write!(f, "Filter[{}]", enabled.join(", "))
    }
}

// -- DiagnosticCodeAction resolver -------------------------------------------

/// A code action suggested for resolving a diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCodeAction {
    pub title: String,
    pub diagnostic_message: String,
    pub edit_description: Option<String>,
    pub is_preferred: bool,
}

impl fmt::Display for DiagnosticCodeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CodeAction({})", self.title)?;
        if self.is_preferred {
            write!(f, " [preferred]")?;
        }
        Ok(())
    }
}

/// Resolve code actions for a diagnostic by checking its code.
pub fn resolve_code_actions(diag: &Diagnostic) -> Vec<DiagnosticCodeAction> {
    let mut actions = Vec::new();
    if let Some(code) = &diag.code {
        actions.push(DiagnosticCodeAction {
            title: format!("Fix: {}", diag.message),
            diagnostic_message: diag.message.clone(),
            edit_description: Some(format!("Resolve {code}")),
            is_preferred: true,
        });
    }
    if diag.tags.contains(&DiagnosticTag::Unnecessary) {
        actions.push(DiagnosticCodeAction {
            title: "Remove unused code".to_string(),
            diagnostic_message: diag.message.clone(),
            edit_description: Some("Remove unnecessary code".to_string()),
            is_preferred: false,
        });
    }
    if diag.tags.contains(&DiagnosticTag::Deprecated) {
        actions.push(DiagnosticCodeAction {
            title: "Update deprecated usage".to_string(),
            diagnostic_message: diag.message.clone(),
            edit_description: None,
            is_preferred: false,
        });
    }
    actions
}

// -- Diagnostic change delta computation ------------------------------------

/// Represents a change in diagnostics for a URI.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticDelta {
    pub uri: String,
    pub added: Vec<Diagnostic>,
    pub removed: Vec<Diagnostic>,
    pub unchanged: usize,
}

impl DiagnosticDelta {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }

    pub fn added_count(&self) -> usize {
        self.added.len()
    }

    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }
}

impl fmt::Display for DiagnosticDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Delta({}): +{} -{} ={} unchanged",
            self.uri,
            self.added.len(),
            self.removed.len(),
            self.unchanged,
        )
    }
}

/// Compute the delta between old and new diagnostics for a URI.
pub fn compute_diagnostic_delta(
    uri: &str,
    old: &[Diagnostic],
    new: &[Diagnostic],
) -> DiagnosticDelta {
    let mut added = Vec::new();
    let mut unchanged = 0usize;

    for n in new {
        if old.contains(n) {
            unchanged += 1;
        } else {
            added.push(n.clone());
        }
    }

    let removed: Vec<Diagnostic> = old.iter().filter(|o| !new.contains(o)).cloned().collect();

    DiagnosticDelta {
        uri: uri.to_string(),
        added,
        removed,
        unchanged,
    }
}

/// Compute deltas for an entire collection compared to another.
pub fn compute_collection_deltas(
    old: &DiagnosticCollection,
    new: &DiagnosticCollection,
) -> Vec<DiagnosticDelta> {
    let mut all_uris: Vec<&String> = old.entries.keys().chain(new.entries.keys()).collect();
    all_uris.sort();
    all_uris.dedup();

    all_uris
        .into_iter()
        .map(|uri| {
            let empty = Vec::new();
            let old_diags = old.entries.get(uri).unwrap_or(&empty);
            let new_diags = new.entries.get(uri).unwrap_or(&empty);
            compute_diagnostic_delta(uri, old_diags, new_diags)
        })
        .filter(|d| d.has_changes())
        .collect()
}

// ---------------------------------------------------------------------------
// DiagnosticCollectionManager - diagnostic collection manager
// ---------------------------------------------------------------------------

/// Severity level for diagnostic collection manager issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticCollectionManagerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for DiagnosticCollectionManagerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [DiagnosticCollectionManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticCollectionManagerEntry {
    pub id: String,
    pub label: String,
    pub severity: DiagnosticCollectionManagerSeverity,
    pub detail: Option<String>,
    pub diagnostic_count: usize,
    enabled: bool,
}

impl DiagnosticCollectionManagerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: DiagnosticCollectionManagerSeverity::Low,
            detail: None,
            diagnostic_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: DiagnosticCollectionManagerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_diagnostic_count(mut self, val: usize) -> Self {
        self.diagnostic_count = val;
        self
    }

    pub fn has_errors(&self) -> bool {
        self.enabled && self.severity >= DiagnosticCollectionManagerSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.diagnostic_count, det)
    }
}

impl fmt::Display for DiagnosticCollectionManagerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [DiagnosticCollectionManagerEntry] items.
#[derive(Debug, Clone)]
pub struct DiagnosticCollectionManager {
    entries: Vec<DiagnosticCollectionManagerEntry>,
    name: String,
    capacity: usize,
}

impl DiagnosticCollectionManager {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: DiagnosticCollectionManagerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<DiagnosticCollectionManagerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&DiagnosticCollectionManagerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn diagnostic_count(&self) -> usize { self.entries.len() }

    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|e| e.has_errors())
    }

    pub fn entries_by_severity(&self, severity: DiagnosticCollectionManagerSeverity) -> Vec<&DiagnosticCollectionManagerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= DiagnosticCollectionManagerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&DiagnosticCollectionManagerEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&DiagnosticCollectionManagerEntry> {
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
// DiagnosticQuickNavigator - diagnostic quick navigate
// ---------------------------------------------------------------------------

/// Configuration for [DiagnosticQuickNavigator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticQuickNavigatorConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub collection_count: usize,
}

impl DiagnosticQuickNavigatorConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, collection_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_collection_count(mut self, val: usize) -> Self { self.collection_count = val; self }
}

impl Default for DiagnosticQuickNavigatorConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [DiagnosticQuickNavigator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticQuickNavigatorItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl DiagnosticQuickNavigatorItem {
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

    pub fn has_next(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for DiagnosticQuickNavigatorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [DiagnosticQuickNavigatorItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct DiagnosticQuickNavigator {
    config: DiagnosticQuickNavigatorConfig,
    items: Vec<DiagnosticQuickNavigatorItem>,
}

impl DiagnosticQuickNavigator {
    pub fn new(config: DiagnosticQuickNavigatorConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: DiagnosticQuickNavigatorItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<DiagnosticQuickNavigatorItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&DiagnosticQuickNavigatorItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn collection_count(&self) -> usize { self.items.len() }

    pub fn has_next(&self) -> bool {
        self.items.iter().any(|i| i.has_next())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&DiagnosticQuickNavigatorItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DiagnosticQuickNavigatorItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &DiagnosticQuickNavigatorConfig {
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
// vsedit-ext-diagnostics: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtDiagnosticsXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtDiagnosticsXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ExtDiagnosticsXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtDiagnosticsXRegistry {
    entries: Vec<ExtDiagnosticsXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtDiagnosticsXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtDiagnosticsXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtDiagnosticsXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtDiagnosticsXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtDiagnosticsXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ExtDiagnosticsXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtDiagnosticsXConfig> {
        let mut sorted: Vec<&ExtDiagnosticsXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtDiagnosticsXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ExtDiagnosticsXIterator<'_> {
        ExtDiagnosticsXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtDiagnosticsXIterator<'a> {
    inner: std::slice::Iter<'a, ExtDiagnosticsXConfig>,
}

impl<'a> Iterator for ExtDiagnosticsXIterator<'a> {
    type Item = &'a ExtDiagnosticsXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtDiagnosticsXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtDiagnosticsXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ExtDiagnosticsXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtDiagnosticsXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ExtDiagnosticsXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtDiagnosticsXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtDiagnosticsXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtDiagnosticsXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtDiagnosticsXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtDiagnosticsXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ExtDiagnosticsXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ExtDiagnosticsXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtDiagnosticsXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 52
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer52 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer52 {
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
pub fn xb_fnv1a_52(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_52<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_52<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_52(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_52(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 56
// ---------------------------------------------------------------------------

/// Generic object pool `Xc56Pool<T>`.
pub struct Xc56Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc56Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc56PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc56Pool<T> {
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
    pub fn stats(&self) -> Xc56PoolStats {
        Xc56PoolStats {
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

impl<T> Default for Xc56Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc56Scheduler`.
pub struct Xc56Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc56Scheduler {
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

impl Default for Xc56Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_56 hash for the given byte slice.
pub fn xc_56_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_56 convention.
pub fn xc_56_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe65 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe65Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe65PipelineError {
    pub stage: Xe65Stage,
    pub message: String,
}

impl std::fmt::Display for Xe65PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe65Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe65Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError>>>,
    stage_names: Vec<Xe65Stage>,
}

impl Xe65Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe65Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe65Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe65Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe65Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
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

    pub fn compose(mut self, other: Xe65Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe65CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe65CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe65Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe65CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe65CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe65Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe65CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_65_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe65CacheEntry {
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

    fn xe_65_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe65CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_65_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
    Ok(data)
}

pub fn xe_65_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_65_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_65_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_65_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe65PipelineError> {
    Err(Xe65PipelineError {
        stage: Xe65Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_63: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg63Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg63Graph {
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

impl Default for Xg63Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_63: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg63Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg63Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg63Heap<T>) {
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

impl<T: Ord> Default for Xg63Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 55).
pub struct Xh55SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh55SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 97 as u64,
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

/// A compact bit set supporting boolean operations (variant 55).
pub struct Xh55BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh55BitSet {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn set_and_get_diagnostics() {
        let mut bridge = DiagnosticBridge::new();
        let diag = Diagnostic {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            message: "unused variable".into(),
            severity: DiagnosticSeverity::Warning,
            code: Some("W001".into()),
            source: Some("rustc".into()),
            related_info: Vec::new(),
            tags: vec![DiagnosticTag::Unnecessary],
        };
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "rust".into(),
            uri: "file:///a.rs".into(),
            diagnostics: vec![diag],
        });
        assert_eq!(bridge.total_diagnostics(), 1);
        let resp = bridge.handle(DiagnosticMessage::GetDiagnostics {
            uri: Some("file:///a.rs".into()),
        });
        if let DiagnosticResponse::Diagnostics { entries } = resp {
            assert_eq!(entries.len(), 1);
        } else {
            panic!("expected Diagnostics");
        }
    }

    #[test]
    fn clear_single_uri() {
        let mut bridge = DiagnosticBridge::new();
        let diag = Diagnostic {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            message: "err".into(), severity: DiagnosticSeverity::Error,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        };
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "c".into(), uri: "file:///a.rs".into(), diagnostics: vec![diag.clone()],
        });
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "c".into(), uri: "file:///b.rs".into(), diagnostics: vec![diag],
        });
        assert_eq!(bridge.total_diagnostics(), 2);
        bridge.handle(DiagnosticMessage::ClearDiagnostics {
            collection: "c".into(), uri: Some("file:///a.rs".into()),
        });
        assert_eq!(bridge.total_diagnostics(), 1);
    }

    #[test]
    fn clear_all_in_collection() {
        let mut bridge = DiagnosticBridge::new();
        let diag = Diagnostic {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            message: "x".into(), severity: DiagnosticSeverity::Hint,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        };
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "c".into(), uri: "file:///a.rs".into(), diagnostics: vec![diag],
        });
        bridge.handle(DiagnosticMessage::ClearDiagnostics {
            collection: "c".into(), uri: None,
        });
        assert_eq!(bridge.total_diagnostics(), 0);
    }

    #[test]
    fn multiple_collections() {
        let mut bridge = DiagnosticBridge::new();
        let diag = Diagnostic {
            start_line: 1, start_col: 0, end_line: 1, end_col: 10,
            message: "info".into(), severity: DiagnosticSeverity::Information,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        };
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "lint".into(), uri: "file:///a.rs".into(), diagnostics: vec![diag.clone()],
        });
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "compiler".into(), uri: "file:///a.rs".into(), diagnostics: vec![diag],
        });
        assert_eq!(bridge.collection_count(), 2);
    }

    #[test]
    fn serde_round_trip() {
        let msg = DiagnosticMessage::SetDiagnostics {
            collection: "test".into(),
            uri: "file:///x.rs".into(),
            diagnostics: vec![Diagnostic {
                start_line: 5, start_col: 0, end_line: 5, end_col: 3,
                message: "unused".into(), severity: DiagnosticSeverity::Warning,
                code: Some("W1".into()), source: Some("clippy".into()),
                related_info: vec![DiagnosticRelatedInfo {
                    uri: "file:///y.rs".into(), start_line: 1, start_col: 0,
                    message: "defined here".into(),
                }],
                tags: vec![DiagnosticTag::Deprecated],
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: DiagnosticMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ── New tests ──

    fn make_diag(msg: &str, severity: DiagnosticSeverity) -> Diagnostic {
        DiagnosticBuilder::new(msg, severity)
            .span(1, 0, 1, 5)
            .build()
            .unwrap()
    }

    #[test]
    fn builder_creates_valid_diagnostic() {
        let diag = DiagnosticBuilder::new("test error", DiagnosticSeverity::Error)
            .span(3, 0, 3, 10)
            .code("E001")
            .source("test-lint")
            .tag(DiagnosticTag::Deprecated)
            .build()
            .unwrap();
        assert_eq!(diag.message, "test error");
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.code.as_deref(), Some("E001"));
        assert_eq!(diag.source.as_deref(), Some("test-lint"));
        assert_eq!(diag.tags, vec![DiagnosticTag::Deprecated]);
    }

    #[test]
    fn builder_rejects_empty_message() {
        let result = DiagnosticBuilder::new("", DiagnosticSeverity::Warning)
            .span(0, 0, 0, 1)
            .build();
        assert_eq!(result, Err(DiagnosticError::EmptyMessage));
    }

    #[test]
    fn builder_rejects_invalid_span() {
        let result = DiagnosticBuilder::new("bad span", DiagnosticSeverity::Error)
            .span(5, 10, 3, 0)
            .build();
        assert!(matches!(result, Err(DiagnosticError::InvalidSpan { .. })));
    }

    #[test]
    fn diagnostic_display() {
        let diag = make_diag("something wrong", DiagnosticSeverity::Error);
        let text = format!("{}", diag);
        assert!(text.contains("error"));
        assert!(text.contains("something wrong"));
    }

    #[test]
    fn severity_ordering() {
        assert!(DiagnosticSeverity::Error < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Information);
        assert!(DiagnosticSeverity::Information < DiagnosticSeverity::Hint);
    }

    #[test]
    fn severity_is_blocking() {
        assert!(DiagnosticSeverity::Error.is_blocking());
        assert!(!DiagnosticSeverity::Warning.is_blocking());
        assert!(!DiagnosticSeverity::Information.is_blocking());
        assert!(!DiagnosticSeverity::Hint.is_blocking());
    }

    #[test]
    fn diagnostic_span_helpers() {
        let single = DiagnosticBuilder::new("x", DiagnosticSeverity::Warning)
            .span(1, 2, 1, 8)
            .build()
            .unwrap();
        assert!(!single.is_multiline());
        assert_eq!(single.span_length(), Some(6));

        let multi = DiagnosticBuilder::new("y", DiagnosticSeverity::Warning)
            .span(1, 0, 3, 5)
            .build()
            .unwrap();
        assert!(multi.is_multiline());
        assert_eq!(multi.span_length(), None);
    }

    #[test]
    fn collection_filter_by_severity() {
        let mut col = DiagnosticCollection::new("test");
        col.entries.insert(
            "file:///a.rs".into(),
            vec![
                make_diag("err1", DiagnosticSeverity::Error),
                make_diag("warn1", DiagnosticSeverity::Warning),
            ],
        );
        col.entries.insert(
            "file:///b.rs".into(),
            vec![make_diag("err2", DiagnosticSeverity::Error)],
        );

        let errors = col.filter_by_severity(DiagnosticSeverity::Error);
        assert_eq!(errors.len(), 2);
        let warnings = col.filter_by_severity(DiagnosticSeverity::Warning);
        assert_eq!(warnings.len(), 1);
        assert!(col.has_errors());
        assert_eq!(col.worst_severity(), Some(DiagnosticSeverity::Error));
    }

    #[test]
    fn handle_validated_rejects_empty_collection() {
        let mut bridge = DiagnosticBridge::new();
        let result = bridge.handle_validated(DiagnosticMessage::SetDiagnostics {
            collection: "".into(),
            uri: "file:///a.rs".into(),
            diagnostics: vec![],
        });
        assert_eq!(result, Err(DiagnosticError::EmptyCollectionName));
    }

    #[test]
    fn handle_validated_rejects_empty_uri() {
        let mut bridge = DiagnosticBridge::new();
        let result = bridge.handle_validated(DiagnosticMessage::SetDiagnostics {
            collection: "test".into(),
            uri: "".into(),
            diagnostics: vec![],
        });
        assert!(matches!(result, Err(DiagnosticError::InvalidUri(_))));
    }

    #[test]
    fn bridge_count_by_severity() {
        let mut bridge = DiagnosticBridge::new();
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "a".into(),
            uri: "file:///x.rs".into(),
            diagnostics: vec![
                make_diag("e1", DiagnosticSeverity::Error),
                make_diag("w1", DiagnosticSeverity::Warning),
            ],
        });
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "b".into(),
            uri: "file:///y.rs".into(),
            diagnostics: vec![make_diag("e2", DiagnosticSeverity::Error)],
        });
        assert_eq!(bridge.count_by_severity(DiagnosticSeverity::Error), 2);
        assert_eq!(bridge.count_by_severity(DiagnosticSeverity::Warning), 1);
        assert!(bridge.has_errors());
        assert_eq!(bridge.collection_names().len(), 2);
    }

    #[test]
    fn error_display_messages() {
        let e = DiagnosticError::EmptyCollectionName;
        assert_eq!(e.to_string(), "collection name must not be empty");

        let e = DiagnosticError::EmptyMessage;
        assert_eq!(e.to_string(), "diagnostic message must not be empty");

        let e = DiagnosticError::InvalidUri("".into());
        assert!(e.to_string().contains("invalid URI"));

        let e = DiagnosticError::InvalidSpan {
            start_line: 5, start_col: 10, end_line: 3, end_col: 0,
        };
        assert!(e.to_string().contains("invalid span"));
    }

    #[test]
    fn severity_aggregation_basic() {
        let diags = vec![
            make_diag("e1", DiagnosticSeverity::Error),
            make_diag("e2", DiagnosticSeverity::Error),
            make_diag("w1", DiagnosticSeverity::Warning),
            make_diag("i1", DiagnosticSeverity::Information),
            make_diag("h1", DiagnosticSeverity::Hint),
        ];
        let agg = SeverityAggregation::from_diagnostics(&diags);
        assert_eq!(agg.errors, 2);
        assert_eq!(agg.warnings, 1);
        assert_eq!(agg.infos, 1);
        assert_eq!(agg.hints, 1);
        assert_eq!(agg.total(), 5);
        assert!(!agg.is_empty());
        assert!(agg.to_string().contains("E:2"));
    }

    #[test]
    fn severity_aggregation_empty() {
        let agg = SeverityAggregation::from_diagnostics(&[]);
        assert!(agg.is_empty());
        assert_eq!(agg.total(), 0);
    }

    #[test]
    fn deduplication_removes_duplicates() {
        let d1 = make_diag("same msg", DiagnosticSeverity::Warning);
        let d2 = make_diag("same msg", DiagnosticSeverity::Warning);
        let d3 = make_diag("different", DiagnosticSeverity::Warning);
        let result = deduplicate_diagnostics(&[d1, d2, d3]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message, "same msg");
        assert_eq!(result[1].message, "different");
    }

    #[test]
    fn ranges_intersect_overlapping() {
        let a = DiagnosticBuilder::new("a", DiagnosticSeverity::Error).span(1, 0, 3, 10).build().unwrap();
        let b = DiagnosticBuilder::new("b", DiagnosticSeverity::Error).span(2, 5, 4, 0).build().unwrap();
        assert!(ranges_intersect(&a, &b));
    }

    #[test]
    fn ranges_intersect_non_overlapping() {
        let a = DiagnosticBuilder::new("a", DiagnosticSeverity::Error).span(1, 0, 1, 10).build().unwrap();
        let b = DiagnosticBuilder::new("b", DiagnosticSeverity::Error).span(2, 0, 2, 10).build().unwrap();
        assert!(!ranges_intersect(&a, &b));
    }

    #[test]
    fn quick_fix_tracker_basic() {
        let mut tracker = QuickFixTracker::new();
        tracker.add(QuickFixSuggestion {
            title: "Add import".into(),
            replacement_text: "use std::fmt;".into(),
            diagnostic_index: 0,
        });
        tracker.add(QuickFixSuggestion {
            title: "Remove unused".into(),
            replacement_text: "".into(),
            diagnostic_index: 1,
        });
        tracker.add(QuickFixSuggestion {
            title: "Rename".into(),
            replacement_text: "new_name".into(),
            diagnostic_index: 0,
        });
        assert_eq!(tracker.total_suggestions(), 3);
        assert_eq!(tracker.suggestions_for(0).len(), 2);
        assert_eq!(tracker.suggestions_for(1).len(), 1);
        assert_eq!(tracker.suggestions_for(99).len(), 0);
        tracker.clear();
        assert_eq!(tracker.total_suggestions(), 0);
    }

    // ---- DiagnosticSeverityCounter tests ----

    #[test]
    fn counter_increment_and_get() {
        let mut counter = DiagnosticSeverityCounter::new();
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Warning);
        assert_eq!(counter.get(DiagnosticSeverity::Error), 2);
        assert_eq!(counter.get(DiagnosticSeverity::Warning), 1);
        assert_eq!(counter.get(DiagnosticSeverity::Information), 0);
        assert_eq!(counter.total(), 3);
    }

    #[test]
    fn counter_decrement_saturates() {
        let mut counter = DiagnosticSeverityCounter::new();
        counter.decrement(DiagnosticSeverity::Error);
        assert_eq!(counter.errors, 0); // no underflow
        counter.increment(DiagnosticSeverity::Hint);
        counter.increment(DiagnosticSeverity::Hint);
        counter.decrement(DiagnosticSeverity::Hint);
        assert_eq!(counter.hints, 1);
    }

    #[test]
    fn counter_worst_severity() {
        let mut counter = DiagnosticSeverityCounter::new();
        assert!(counter.worst_severity().is_none());
        counter.increment(DiagnosticSeverity::Hint);
        assert_eq!(counter.worst_severity(), Some(DiagnosticSeverity::Hint));
        counter.increment(DiagnosticSeverity::Warning);
        assert_eq!(counter.worst_severity(), Some(DiagnosticSeverity::Warning));
        counter.increment(DiagnosticSeverity::Error);
        assert_eq!(counter.worst_severity(), Some(DiagnosticSeverity::Error));
    }

    #[test]
    fn counter_reset() {
        let mut counter = DiagnosticSeverityCounter::new();
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Warning);
        counter.reset();
        assert_eq!(counter.total(), 0);
    }

    #[test]
    fn counter_display() {
        let mut counter = DiagnosticSeverityCounter::new();
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Warning);
        let s = format!("{counter}");
        assert!(s.contains("errors=1"));
        assert!(s.contains("warnings=1"));
    }

    // ---- filter_diagnostics tests ----

    #[test]
    fn filter_by_severity_only() {
        let diags = vec![
            make_diag("e1", DiagnosticSeverity::Error),
            make_diag("w1", DiagnosticSeverity::Warning),
            make_diag("e2", DiagnosticSeverity::Error),
        ];
        let filtered = filter_diagnostics(&diags, Some(DiagnosticSeverity::Error), None);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|d| d.severity == DiagnosticSeverity::Error));
    }

    #[test]
    fn filter_by_source_only() {
        let mut d1 = make_diag("e1", DiagnosticSeverity::Error);
        d1.source = Some("rustc".into());
        let mut d2 = make_diag("e2", DiagnosticSeverity::Error);
        d2.source = Some("clippy".into());
        let mut d3 = make_diag("w1", DiagnosticSeverity::Warning);
        d3.source = Some("RUSTC".into()); // case-insensitive
        let diags = vec![d1, d2, d3];
        let filtered = filter_diagnostics(&diags, None, Some("rustc"));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_severity_and_source() {
        let mut d1 = make_diag("e1", DiagnosticSeverity::Error);
        d1.source = Some("rustc".into());
        let mut d2 = make_diag("w1", DiagnosticSeverity::Warning);
        d2.source = Some("rustc".into());
        let diags = vec![d1, d2];
        let filtered = filter_diagnostics(&diags, Some(DiagnosticSeverity::Error), Some("rustc"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "e1");
    }

    #[test]
    fn filter_no_source_match() {
        let d1 = make_diag("e1", DiagnosticSeverity::Error); // source is None
        let filtered = filter_diagnostics(&[d1], None, Some("rustc"));
        assert!(filtered.is_empty());
    }

    // ---- diagnostic_sort tests ----

    #[test]
    fn sort_by_severity_then_line() {
        let mut diags = vec![
            DiagnosticBuilder::new("hint", DiagnosticSeverity::Hint)
                .span(1, 0, 1, 5).build().unwrap(),
            DiagnosticBuilder::new("error-line5", DiagnosticSeverity::Error)
                .span(5, 0, 5, 5).build().unwrap(),
            DiagnosticBuilder::new("warning", DiagnosticSeverity::Warning)
                .span(3, 0, 3, 5).build().unwrap(),
            DiagnosticBuilder::new("error-line2", DiagnosticSeverity::Error)
                .span(2, 0, 2, 5).build().unwrap(),
        ];
        diagnostic_sort(&mut diags);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diags[0].start_line, 2);
        assert_eq!(diags[1].severity, DiagnosticSeverity::Error);
        assert_eq!(diags[1].start_line, 5);
        assert_eq!(diags[2].severity, DiagnosticSeverity::Warning);
        assert_eq!(diags[3].severity, DiagnosticSeverity::Hint);
    }

    #[test]
    fn sort_by_column_within_same_line_severity() {
        let mut diags = vec![
            DiagnosticBuilder::new("b", DiagnosticSeverity::Error)
                .span(1, 10, 1, 15).build().unwrap(),
            DiagnosticBuilder::new("a", DiagnosticSeverity::Error)
                .span(1, 2, 1, 8).build().unwrap(),
        ];
        diagnostic_sort(&mut diags);
        assert_eq!(diags[0].start_col, 2);
        assert_eq!(diags[1].start_col, 10);
    }

    #[test]
    fn diagnostic_collection_clear() {
        let mut col = DiagnosticCollection::new("test");
        col.entries.insert("file:///a.rs".into(), vec![Diagnostic {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            message: "err".into(), severity: DiagnosticSeverity::Error,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        }]);
        assert_eq!(col.diagnostic_count(), 1);
        col.clear();
        assert_eq!(col.diagnostic_count(), 0);
    }

    #[test]
    fn diagnostic_collection_all_diagnostics() {
        let mut col = DiagnosticCollection::new("test");
        col.entries.insert("file:///a.rs".into(), vec![
            Diagnostic {
                start_line: 0, start_col: 0, end_line: 0, end_col: 1,
                message: "e1".into(), severity: DiagnosticSeverity::Error,
                code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
            },
        ]);
        col.entries.insert("file:///b.rs".into(), vec![
            Diagnostic {
                start_line: 1, start_col: 0, end_line: 1, end_col: 5,
                message: "w1".into(), severity: DiagnosticSeverity::Warning,
                code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
            },
        ]);
        let all = col.all_diagnostics();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn diagnostic_bridge_summary_and_display() {
        let mut bridge = DiagnosticBridge::new();
        bridge.handle(DiagnosticMessage::SetDiagnostics {
            collection: "rust".into(),
            uri: "file:///a.rs".into(),
            diagnostics: vec![Diagnostic {
                start_line: 0, start_col: 0, end_line: 0, end_col: 1,
                message: "err".into(), severity: DiagnosticSeverity::Error,
                code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
            }],
        });
        assert_eq!(bridge.summary(), "1 diagnostic(s) in 1 collection(s)");
        assert_eq!(format!("{bridge}"), "1 diagnostic(s) in 1 collection(s)");
    }

    #[test]
    fn diagnostic_severity_label() {
        let d = Diagnostic {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            message: "m".into(), severity: DiagnosticSeverity::Warning,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        };
        assert_eq!(d.severity_label(), "warning");
        assert!(d.is_warning());
        assert!(!d.is_hint());
    }

    #[test]
    fn diagnostic_is_hint() {
        let d = Diagnostic {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            message: "h".into(), severity: DiagnosticSeverity::Hint,
            code: None, source: None, related_info: Vec::new(), tags: Vec::new(),
        };
        assert!(d.is_hint());
        assert!(!d.is_warning());
        assert_eq!(d.severity_label(), "hint");
    }

    #[test]
    fn severity_aggregation_worst() {
        let empty = SeverityAggregation::default();
        assert!(empty.worst().is_none());

        let hints_only = SeverityAggregation { errors: 0, warnings: 0, infos: 0, hints: 2 };
        assert_eq!(hints_only.worst(), Some(DiagnosticSeverity::Hint));

        let mixed = SeverityAggregation { errors: 1, warnings: 3, infos: 0, hints: 0 };
        assert_eq!(mixed.worst(), Some(DiagnosticSeverity::Error));

        let warns = SeverityAggregation { errors: 0, warnings: 1, infos: 2, hints: 0 };
        assert_eq!(warns.worst(), Some(DiagnosticSeverity::Warning));
    }

    #[test]
    fn diagnostic_bridge_empty_summary() {
        let bridge = DiagnosticBridge::new();
        assert_eq!(bridge.summary(), "0 diagnostic(s) in 0 collection(s)");
    }

    // ---- DiagnosticFilter tests ----

    #[test]
    fn filter_builder_severity_range() {
        let diags = vec![
            make_diag("e1", DiagnosticSeverity::Error),
            make_diag("w1", DiagnosticSeverity::Warning),
            make_diag("i1", DiagnosticSeverity::Information),
            make_diag("h1", DiagnosticSeverity::Hint),
        ];
        // Only errors and warnings (weight 0..=1)
        let filter = DiagnosticFilter::new()
            .min_severity(DiagnosticSeverity::Warning);
        let matched = filter.apply(&diags);
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().all(|d| d.severity == DiagnosticSeverity::Error || d.severity == DiagnosticSeverity::Warning));

        // Only info and hints (weight 2..=3)
        let filter2 = DiagnosticFilter::new()
            .max_severity(DiagnosticSeverity::Information);
        let matched2 = filter2.apply(&diags);
        assert_eq!(matched2.len(), 2);
        assert!(matched2.iter().all(|d| d.severity == DiagnosticSeverity::Information || d.severity == DiagnosticSeverity::Hint));
    }

    #[test]
    fn filter_builder_source_and_file_pattern() {
        let mut d1 = make_diag("e1", DiagnosticSeverity::Error);
        d1.source = Some("rustc".into());
        let mut d2 = make_diag("e2", DiagnosticSeverity::Error);
        d2.source = Some("clippy".into());
        let mut d3 = make_diag("w1", DiagnosticSeverity::Warning);
        d3.source = Some("Rustc".into());

        let mut col = DiagnosticCollection::new("test");
        col.entries.insert("file:///src/main.rs".into(), vec![d1, d2]);
        col.entries.insert("file:///tests/foo.rs".into(), vec![d3]);

        let filter = DiagnosticFilter::new()
            .source("rustc")
            .file_pattern("/src/");
        let matched = filter.apply_to_collection(&col);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].1.message, "e1");
    }

    #[test]
    fn filter_builder_tags() {
        let mut d1 = make_diag("deprecated", DiagnosticSeverity::Warning);
        d1.tags = vec![DiagnosticTag::Deprecated];
        let mut d2 = make_diag("unnecessary", DiagnosticSeverity::Hint);
        d2.tags = vec![DiagnosticTag::Unnecessary];
        let d3 = make_diag("plain", DiagnosticSeverity::Error);

        let diags = vec![d1, d2, d3];

        let require_dep = DiagnosticFilter::new().require_tag(DiagnosticTag::Deprecated);
        assert_eq!(require_dep.apply(&diags).len(), 1);
        assert_eq!(require_dep.apply(&diags)[0].message, "deprecated");

        let exclude_dep = DiagnosticFilter::new().exclude_tag(DiagnosticTag::Deprecated);
        assert_eq!(exclude_dep.apply(&diags).len(), 2);
    }

    #[test]
    fn filter_display() {
        let filter = DiagnosticFilter::new()
            .min_severity(DiagnosticSeverity::Warning)
            .source("rustc");
        let s = format!("{filter}");
        assert!(s.contains("min_severity=warning"));
        assert!(s.contains("rustc"));

        let empty_filter = DiagnosticFilter::new();
        assert_eq!(format!("{empty_filter}"), "DiagnosticFilter(all)");
    }

    // ---- DiagnosticDiff tests ----

    #[test]
    fn diff_added_removed_unchanged() {
        let d1 = make_diag("stays", DiagnosticSeverity::Error);
        let d2 = make_diag("goes away", DiagnosticSeverity::Warning);
        let d3 = make_diag("brand new", DiagnosticSeverity::Hint);

        let before = vec![d1.clone(), d2];
        let after = vec![d1, d3];
        let diff = DiagnosticDiff::compute(&before, &after);

        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.unchanged[0].message, "stays");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].message, "goes away");
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].message, "brand new");
        assert_eq!(diff.change_count(), 2);
        assert!(!diff.is_empty());

        let s = format!("{diff}");
        assert!(s.contains("+1"));
        assert!(s.contains("-1"));
    }

    #[test]
    fn diff_collections() {
        let mut before = DiagnosticCollection::new("before");
        before.entries.insert(
            "file:///a.rs".into(),
            vec![make_diag("old", DiagnosticSeverity::Error)],
        );

        let mut after = DiagnosticCollection::new("after");
        after.entries.insert(
            "file:///a.rs".into(),
            vec![make_diag("new", DiagnosticSeverity::Warning)],
        );
        after.entries.insert(
            "file:///b.rs".into(),
            vec![make_diag("added file", DiagnosticSeverity::Hint)],
        );

        let diffs = DiagnosticDiff::compute_collections(&before, &after);
        assert!(diffs.contains_key("file:///a.rs"));
        assert!(diffs.contains_key("file:///b.rs"));
        assert_eq!(diffs["file:///a.rs"].removed[0].message, "old");
        assert_eq!(diffs["file:///a.rs"].added[0].message, "new");
        assert_eq!(diffs["file:///b.rs"].added[0].message, "added file");
    }

    // ---- DiagnosticGrouping tests ----

    #[test]
    fn grouping_by_severity_and_source() {
        let mut d1 = make_diag("e1", DiagnosticSeverity::Error);
        d1.source = Some("rustc".into());
        let mut d2 = make_diag("e2", DiagnosticSeverity::Error);
        d2.source = Some("clippy".into());
        let mut d3 = make_diag("w1", DiagnosticSeverity::Warning);
        d3.source = Some("rustc".into());
        let d4 = make_diag("h1", DiagnosticSeverity::Hint);

        let diags = vec![d1, d2, d3, d4];

        let by_sev = DiagnosticGrouping::by_severity(&diags);
        assert_eq!(by_sev.group_count(), 3); // error, warning, hint
        assert_eq!(by_sev.total(), 4);
        let summary = by_sev.summary();
        assert_eq!(summary[0].0, "error");
        assert_eq!(summary[0].1, 2);

        let by_src = DiagnosticGrouping::by_source(&diags);
        assert_eq!(by_src.group_count(), 3); // rustc, clippy, <unknown>
        assert_eq!(by_src.total(), 4);
        let (largest_key, largest_count) = by_src.largest_group().unwrap();
        assert_eq!(largest_key, "rustc");
        assert_eq!(largest_count, 2);

        // Display works
        let display = format!("{by_sev}");
        assert!(display.contains("error: 2"));
    }

    // ---- merge_collections tests ----

    #[test]
    fn merge_two_collections() {
        let mut a = DiagnosticCollection::new("lint");
        a.entries.insert(
            "file:///a.rs".into(),
            vec![make_diag("lint-err", DiagnosticSeverity::Warning)],
        );
        a.entries.insert(
            "file:///c.rs".into(),
            vec![make_diag("only-a", DiagnosticSeverity::Hint)],
        );

        let mut b = DiagnosticCollection::new("compiler");
        b.entries.insert(
            "file:///a.rs".into(),
            vec![make_diag("compile-err", DiagnosticSeverity::Error)],
        );
        b.entries.insert(
            "file:///b.rs".into(),
            vec![make_diag("only-b", DiagnosticSeverity::Information)],
        );

        let merged = merge_collections(&a, &b, "merged");
        assert_eq!(merged.name, "merged");
        assert_eq!(merged.uri_count(), 3);
        // file:///a.rs has both diagnostics concatenated
        assert_eq!(merged.entries["file:///a.rs"].len(), 2);
        assert_eq!(merged.entries["file:///b.rs"].len(), 1);
        assert_eq!(merged.entries["file:///c.rs"].len(), 1);
        assert_eq!(merged.diagnostic_count(), 4);
    }

    // ---- From impl tests ----

    #[test]
    fn convert_counter_to_aggregation_roundtrip() {
        let mut counter = DiagnosticSeverityCounter::new();
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Error);
        counter.increment(DiagnosticSeverity::Warning);
        counter.increment(DiagnosticSeverity::Hint);

        let agg: SeverityAggregation = counter.clone().into();
        assert_eq!(agg.errors, 2);
        assert_eq!(agg.warnings, 1);
        assert_eq!(agg.hints, 1);
        assert_eq!(agg.total(), 4);

        let back: DiagnosticSeverityCounter = agg.into();
        assert_eq!(back, counter);
    }

    // -- DiagnosticSeverityFilter tests ---------------------------------------

    #[test]
    fn filter_all_passes_everything() {
        let filter = DiagnosticSeverityFilter::all();
        assert!(filter.matches(&DiagnosticSeverity::Error));
        assert!(filter.matches(&DiagnosticSeverity::Hint));
        assert_eq!(filter.enabled_count(), 4);
    }

    #[test]
    fn filter_errors_only() {
        let filter = DiagnosticSeverityFilter::errors_only();
        assert!(filter.matches(&DiagnosticSeverity::Error));
        assert!(!filter.matches(&DiagnosticSeverity::Warning));
        assert_eq!(filter.enabled_count(), 1);
    }

    #[test]
    fn filter_apply_diagnostics() {
        let filter = DiagnosticSeverityFilter::errors_only();
        let diags = vec![
            make_diag("err", DiagnosticSeverity::Error),
            make_diag("warn", DiagnosticSeverity::Warning),
        ];
        let filtered = filter.apply(&diags);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "err");
    }

    #[test]
    fn severity_filter_display() {
        let filter = DiagnosticSeverityFilter::errors_only();
        let s = format!("{filter}");
        assert!(s.contains("errors"));
        assert!(!s.contains("warnings"));
    }

    // -- DiagnosticCodeAction tests -------------------------------------------

    #[test]
    fn code_action_with_code() {
        let mut diag = make_diag("test error", DiagnosticSeverity::Error);
        diag.code = Some("E001".to_string());
        let actions = resolve_code_actions(&diag);
        assert_eq!(actions.len(), 1);
        assert!(actions[0].is_preferred);
        assert!(actions[0].title.contains("Fix"));
    }

    #[test]
    fn code_action_with_unnecessary_tag() {
        let mut diag = make_diag("unused var", DiagnosticSeverity::Warning);
        diag.tags = vec![DiagnosticTag::Unnecessary];
        let actions = resolve_code_actions(&diag);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Remove unused code");
    }

    #[test]
    fn code_action_display() {
        let action = DiagnosticCodeAction {
            title: "Fix it".into(),
            diagnostic_message: "msg".into(),
            edit_description: None,
            is_preferred: true,
        };
        let s = format!("{action}");
        assert!(s.contains("Fix it"));
        assert!(s.contains("[preferred]"));
    }

    // -- DiagnosticDelta tests ------------------------------------------------

    #[test]
    fn delta_no_changes() {
        let diags = vec![make_diag("same", DiagnosticSeverity::Error)];
        let delta = compute_diagnostic_delta("file:///a.rs", &diags, &diags);
        assert!(!delta.has_changes());
        assert_eq!(delta.unchanged, 1);
    }

    #[test]
    fn delta_added_and_removed() {
        let old = vec![make_diag("old", DiagnosticSeverity::Error)];
        let new = vec![make_diag("new", DiagnosticSeverity::Warning)];
        let delta = compute_diagnostic_delta("file:///a.rs", &old, &new);
        assert!(delta.has_changes());
        assert_eq!(delta.added_count(), 1);
        assert_eq!(delta.removed_count(), 1);
    }

    #[test]
    fn delta_display() {
        let delta = DiagnosticDelta {
            uri: "file:///a.rs".to_string(),
            added: vec![make_diag("new", DiagnosticSeverity::Error)],
            removed: vec![],
            unchanged: 2,
        };
        let s = format!("{delta}");
        assert!(s.contains("+1"));
        assert!(s.contains("-0"));
        assert!(s.contains("2 unchanged"));
    }

    #[test]
    fn collection_deltas_filters_unchanged() {
        let mut old = DiagnosticCollection { name: "old".into(), entries: HashMap::new() };
        let mut new_col = DiagnosticCollection { name: "new".into(), entries: HashMap::new() };
        let d = make_diag("same", DiagnosticSeverity::Error);
        old.entries.insert("file:///a.rs".into(), vec![d.clone()]);
        new_col.entries.insert("file:///a.rs".into(), vec![d]);
        let deltas = compute_collection_deltas(&old, &new_col);
        assert!(deltas.is_empty());
    }

#[test]
    fn diagnosticcollectionmanager_severity_ordering() {
        assert!(DiagnosticCollectionManagerSeverity::Critical > DiagnosticCollectionManagerSeverity::High);
        assert!(DiagnosticCollectionManagerSeverity::High > DiagnosticCollectionManagerSeverity::Medium);
        assert!(DiagnosticCollectionManagerSeverity::Medium > DiagnosticCollectionManagerSeverity::Low);
    }

    #[test]
    fn diagnosticcollectionmanager_severity_display() {
        assert_eq!(DiagnosticCollectionManagerSeverity::Low.to_string(), "low");
        assert_eq!(DiagnosticCollectionManagerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn diagnosticcollectionmanager_entry_creation() {
        let e = DiagnosticCollectionManagerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, DiagnosticCollectionManagerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn diagnosticcollectionmanager_entry_builder() {
        let e = DiagnosticCollectionManagerEntry::new("e2", "Entry 2")
            .with_severity(DiagnosticCollectionManagerSeverity::High)
            .with_detail("some detail")
            .with_diagnostic_count(42);
        assert_eq!(e.severity, DiagnosticCollectionManagerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.diagnostic_count, 42);
    }

    #[test]
    fn diagnosticcollectionmanager_entry_enable_disable() {
        let mut e = DiagnosticCollectionManagerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn diagnosticcollectionmanager_add_and_count() {
        let mut mgr = DiagnosticCollectionManager::new("test");
        mgr.add(DiagnosticCollectionManagerEntry::new("a", "A"));
        mgr.add(DiagnosticCollectionManagerEntry::new("b", "B").with_severity(DiagnosticCollectionManagerSeverity::High));
        assert_eq!(mgr.diagnostic_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn diagnosticcollectionmanager_remove() {
        let mut mgr = DiagnosticCollectionManager::new("test");
        mgr.add(DiagnosticCollectionManagerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn diagnosticcollectionmanager_capacity() {
        let mut mgr = DiagnosticCollectionManager::new("test").with_capacity(1);
        assert!(mgr.add(DiagnosticCollectionManagerEntry::new("a", "A")));
        assert!(!mgr.add(DiagnosticCollectionManagerEntry::new("b", "B")));
    }

    #[test]
    fn diagnosticcollectionmanager_sorted_by_severity() {
        let mut mgr = DiagnosticCollectionManager::new("test");
        mgr.add(DiagnosticCollectionManagerEntry::new("lo", "Low"));
        mgr.add(DiagnosticCollectionManagerEntry::new("hi", "High").with_severity(DiagnosticCollectionManagerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, DiagnosticCollectionManagerSeverity::Critical);
    }

    #[test]
    fn diagnosticcollectionmanager_summary() {
        let mgr = DiagnosticCollectionManager::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn diagnosticquicknavigator_config_defaults() {
        let cfg = DiagnosticQuickNavigatorConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn diagnosticquicknavigator_item_creation() {
        let item = DiagnosticQuickNavigatorItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn diagnosticquicknavigator_add_and_get() {
        let mut mgr = DiagnosticQuickNavigator::new(DiagnosticQuickNavigatorConfig::new("test"));
        mgr.add(DiagnosticQuickNavigatorItem::new("k1", "v1"));
        assert_eq!(mgr.collection_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn diagnosticquicknavigator_remove_item() {
        let mut mgr = DiagnosticQuickNavigator::new(DiagnosticQuickNavigatorConfig::new("test"));
        mgr.add(DiagnosticQuickNavigatorItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn diagnosticquicknavigator_sorted_by_priority() {
        let mut mgr = DiagnosticQuickNavigator::new(DiagnosticQuickNavigatorConfig::new("test"));
        mgr.add(DiagnosticQuickNavigatorItem::new("lo", "low").with_priority(1));
        mgr.add(DiagnosticQuickNavigatorItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn diagnosticquicknavigator_items_with_tag() {
        let mut mgr = DiagnosticQuickNavigator::new(DiagnosticQuickNavigatorConfig::new("test"));
        mgr.add(DiagnosticQuickNavigatorItem::new("a", "1").with_tag("x"));
        mgr.add(DiagnosticQuickNavigatorItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn diagnosticquicknavigator_report() {
        let mgr = DiagnosticQuickNavigator::new(DiagnosticQuickNavigatorConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn extDiagnostics_x_config_new() {
        let c = ExtDiagnosticsXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extDiagnostics_x_config_builder() {
        let c = ExtDiagnosticsXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn extDiagnostics_x_config_display() {
        let c = ExtDiagnosticsXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extDiagnostics_x_registry_insert_get() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extDiagnostics_x_registry_duplicate() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtDiagnosticsXConfig::new("a")).is_err());
    }

    #[test]
    fn extDiagnostics_x_registry_remove() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a")).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extDiagnostics_x_registry_active_entries() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a")).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extDiagnostics_x_registry_by_weight() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extDiagnostics_x_registry_tags() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extDiagnostics_x_registry_total_weight() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extDiagnostics_x_registry_iterator() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a")).unwrap();
        reg.insert(ExtDiagnosticsXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extDiagnostics_x_cache_put_get() {
        let mut cache = ExtDiagnosticsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extDiagnostics_x_cache_eviction() {
        let mut cache = ExtDiagnosticsXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extDiagnostics_x_cache_lru_order() {
        let mut cache = ExtDiagnosticsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extDiagnostics_x_cache_most_least_recent() {
        let mut cache = ExtDiagnosticsXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extDiagnostics_x_formatter_entry() {
        let e = ExtDiagnosticsXConfig::new("k").with_value("v");
        let fmt = ExtDiagnosticsXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extDiagnostics_x_formatter_summary() {
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtDiagnosticsXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extDiagnostics_x_validator_valid() {
        let v = ExtDiagnosticsXValidator::new();
        let c = ExtDiagnosticsXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extDiagnostics_x_validator_empty_key() {
        let v = ExtDiagnosticsXValidator::new();
        let c = ExtDiagnosticsXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extDiagnostics_x_validator_require_value() {
        let v = ExtDiagnosticsXValidator::new().require_value(true);
        let c = ExtDiagnosticsXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extDiagnostics_x_validator_allowed_tags() {
        let v = ExtDiagnosticsXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtDiagnosticsXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extDiagnostics_x_validator_validate_all() {
        let v = ExtDiagnosticsXValidator::new();
        let mut reg = ExtDiagnosticsXRegistry::new();
        reg.insert(ExtDiagnosticsXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_52_push_and_len() {
        let mut rb = super::XbRingBuffer52::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_52_overwrite() {
        let mut rb = super::XbRingBuffer52::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_52_get_out_of_bounds() {
        let rb = super::XbRingBuffer52::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_52_drain_all() {
        let mut rb = super::XbRingBuffer52::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_52_peek_front_back() {
        let mut rb = super::XbRingBuffer52::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_52_clear() {
        let mut rb = super::XbRingBuffer52::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_52_capacity() {
        let rb = super::XbRingBuffer52::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_52_basic() {
        let h = super::xb_fnv1a_52(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_52(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_52_different_inputs() {
        let h1 = super::xb_fnv1a_52(b"abc");
        let h2 = super::xb_fnv1a_52(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_52_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_52(&data);
        let dec = super::xb_rle_decode_52(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_52_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_52(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_52(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_52_values() {
        assert!((super::xb_clamp_52(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_52(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_52(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_52_values() {
        assert!((super::xb_lerp_52(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_52(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_52(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_52_wrap_around_twice() {
        let mut rb = super::XbRingBuffer52::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 56 ----

    #[test]
    fn xc_56_pool_new_empty() {
        let pool: super::Xc56Pool<i32> = super::Xc56Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_56_pool_release_acquire() {
        let mut pool = super::Xc56Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_56_pool_acquire_empty() {
        let mut pool: super::Xc56Pool<i32> = super::Xc56Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_56_pool_full() {
        let mut pool = super::Xc56Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_56_pool_drain() {
        let mut pool = super::Xc56Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_56_pool_stats() {
        let mut pool = super::Xc56Pool::new(8);
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
    fn xc_56_pool_clear() {
        let mut pool = super::Xc56Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_56_pool_shrink() {
        let mut pool = super::Xc56Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_56_pool_default() {
        let pool: super::Xc56Pool<String> = super::Xc56Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_56_pool_extend() {
        let mut pool = super::Xc56Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_56_pool_retain() {
        let mut pool = super::Xc56Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_56_scheduler_round_robin() {
        let mut sched = super::Xc56Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_56_scheduler_empty() {
        let mut sched = super::Xc56Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_56_scheduler_reset() {
        let mut sched = super::Xc56Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_56_scheduler_add_remove() {
        let mut sched = super::Xc56Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_56_scheduler_targets() {
        let sched = super::Xc56Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_56_hash_empty() {
        assert_eq!(super::xc_56_hash(b""), 5381);
    }

    #[test]
    fn xc_56_hash_data() {
        let h = super::xc_56_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_56_hash(b"hello"), h);
    }

    #[test]
    fn xc_56_reverse_str() {
        assert_eq!(super::xc_56_reverse("abc"), "cba");
        assert_eq!(super::xc_56_reverse(""), "");
    }


    #[test]
    fn xe_65_pipeline_empty() {
        let p = super::Xe65Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_65_pipeline_parse_stage() {
        let p = super::Xe65Pipeline::new()
            .add_parse(super::xe_65_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_65_pipeline_transform_double() {
        let p = super::Xe65Pipeline::new()
            .add_transform(super::xe_65_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_65_pipeline_validate_reverse() {
        let p = super::Xe65Pipeline::new()
            .add_validate(super::xe_65_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_65_pipeline_emit_filter() {
        let p = super::Xe65Pipeline::new()
            .add_emit(super::xe_65_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_65_pipeline_multi_stage() {
        let p = super::Xe65Pipeline::new()
            .add_parse(super::xe_65_pipeline_identity)
            .add_transform(super::xe_65_pipeline_double)
            .add_validate(super::xe_65_pipeline_reverse)
            .add_emit(super::xe_65_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_65_pipeline_error_propagation() {
        let p = super::Xe65Pipeline::new()
            .add_parse(super::xe_65_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe65Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_65_pipeline_compose() {
        let p1 = super::Xe65Pipeline::new()
            .add_parse(super::xe_65_pipeline_identity);
        let p2 = super::Xe65Pipeline::new()
            .add_transform(super::xe_65_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_65_pipeline_error_display() {
        let e = super::Xe65PipelineError {
            stage: super::Xe65Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_65_cache_put_get() {
        let mut c = super::Xe65Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_65_cache_miss() {
        let mut c: super::Xe65Cache<&str, i32> = super::Xe65Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_65_cache_ttl_expiry() {
        let mut c = super::Xe65Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_65_cache_evict() {
        let mut c = super::Xe65Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_65_cache_capacity() {
        let mut c = super::Xe65Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_65_cache_stats() {
        let mut c = super::Xe65Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_65_cache_clear() {
        let mut c = super::Xe65Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_63 graph tests ------------------------------------------------

    #[test]
    fn xg_63_graph_empty() {
        let g = super::Xg63Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_63_graph_add_node() {
        let mut g = super::Xg63Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_63_graph_add_edge() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_63_graph_neighbors() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_63_graph_has_path() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_63_graph_self_path() {
        let g = super::Xg63Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_63_graph_topo_sort() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_63_graph_cycle_detect_false() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_63_graph_cycle_detect_true() {
        let mut g = super::Xg63Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_63 heap tests -------------------------------------------------

    #[test]
    fn xg_63_heap_empty() {
        let h: super::Xg63Heap<i32> = super::Xg63Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_63_heap_push_pop() {
        let mut h = super::Xg63Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_63_heap_peek() {
        let mut h = super::Xg63Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_63_heap_drain_sorted() {
        let mut h = super::Xg63Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_63_heap_merge() {
        let mut a = super::Xg63Heap::new();
        let mut b = super::Xg63Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_63_heap_default() {
        let h: super::Xg63Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_63_graph_default() {
        let g: super::Xg63Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh55_skip_insert_contains() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh55_skip_remove() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh55_skip_len() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh55_skip_range_query() {
        let mut sl = super::Xh55SkipList::xh_new(4);
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
    fn xh55_skip_floor_ceiling() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh55_skip_rank() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh55_skip_empty() {
        let sl = super::Xh55SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh55_skip_duplicates() {
        let mut sl = super::Xh55SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh55_bitset_set_test() {
        let mut bs = super::Xh55BitSet::xh_new(256);
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
    fn xh55_bitset_clear_count() {
        let mut bs = super::Xh55BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh55_bitset_and_or_xor() {
        let mut a = super::Xh55BitSet::xh_new(128);
        let mut b = super::Xh55BitSet::xh_new(128);
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
    fn xh55_bitset_iter_ones() {
        let mut bs = super::Xh55BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh55_bitset_first_last() {
        let mut bs = super::Xh55BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh55_bitset_empty() {
        let bs = super::Xh55BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
