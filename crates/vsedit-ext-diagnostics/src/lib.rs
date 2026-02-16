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
}
