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
}
