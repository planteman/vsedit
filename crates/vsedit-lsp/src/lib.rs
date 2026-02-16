//! LSP client integration for language server communication.
//!
//! Provides a JSON-RPC transport, an [`LspClient`](client::LspClient) for
//! communicating with a single language server, and an
//! [`LspManager`](manager::LspManager) that manages multiple servers (one per
//! language).

pub mod client;
pub mod manager;
pub mod transport;

use std::fmt;
pub use client::{LspClient, LspServerConfig};
pub use manager::LspManager;

/// Errors produced by the LSP subsystem.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("failed to spawn server: {0}")]
    SpawnFailed(String),
    #[error("server stdin not available")]
    NoStdin,
    #[error("server stdout not available")]
    NoStdout,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("response channel closed")]
    ResponseChannelClosed,
    #[error("server error {code}: {message}")]
    ServerError { code: i64, message: String },
    #[error("invalid URI: {0}")]
    InvalidUri(String),
    #[error("failed to deserialize: {0}")]
    DeserializeFailed(String),
    #[error("no config registered for language: {0}")]
    NoConfig(String),
}

// ---------------------------------------------------------------------------
// Diagnostic severity helpers
// ---------------------------------------------------------------------------

/// Simplified diagnostic severity levels for display in a terminal editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    /// Convert from an `lsp_types::DiagnosticSeverity` value.
    pub fn from_lsp(sev: lsp_types::DiagnosticSeverity) -> Self {
        match sev {
            lsp_types::DiagnosticSeverity::ERROR => Severity::Error,
            lsp_types::DiagnosticSeverity::WARNING => Severity::Warning,
            lsp_types::DiagnosticSeverity::INFORMATION => Severity::Info,
            lsp_types::DiagnosticSeverity::HINT => Severity::Hint,
            _ => Severity::Info,
        }
    }

    /// Convert back to an `lsp_types::DiagnosticSeverity`.
    pub fn to_lsp(self) -> lsp_types::DiagnosticSeverity {
        match self {
            Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
            Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
            Severity::Info => lsp_types::DiagnosticSeverity::INFORMATION,
            Severity::Hint => lsp_types::DiagnosticSeverity::HINT,
        }
    }

    /// Return a human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        }
    }

    /// Return a short single-character symbol for gutter display.
    pub fn symbol(self) -> char {
        match self {
            Severity::Error => 'E',
            Severity::Warning => 'W',
            Severity::Info => 'I',
            Severity::Hint => 'H',
        }
    }

    /// Return true if this severity is at least as severe as the given level.
    pub fn at_least(self, min: Severity) -> bool {
        self.rank() <= min.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
            Severity::Hint => 3,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// Document position utilities
// ---------------------------------------------------------------------------

/// A zero-indexed position within a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocPosition {
    pub line: u32,
    pub col: u32,
}

impl DocPosition {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    /// Convert to an LSP `Position`.
    pub fn to_lsp(self) -> lsp_types::Position {
        lsp_types::Position {
            line: self.line,
            character: self.col,
        }
    }

    /// Convert from an LSP `Position`.
    pub fn from_lsp(pos: lsp_types::Position) -> Self {
        Self {
            line: pos.line,
            col: pos.character,
        }
    }
}

impl PartialOrd for DocPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DocPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

impl std::fmt::Display for DocPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.col + 1)
    }
}

/// A range within a text document (start inclusive, end exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocRange {
    pub start: DocPosition,
    pub end: DocPosition,
}

impl DocRange {
    pub fn new(start: DocPosition, end: DocPosition) -> Self {
        Self { start, end }
    }

    /// Convert to an LSP `Range`.
    pub fn to_lsp(self) -> lsp_types::Range {
        lsp_types::Range {
            start: self.start.to_lsp(),
            end: self.end.to_lsp(),
        }
    }

    /// Convert from an LSP `Range`.
    pub fn from_lsp(range: lsp_types::Range) -> Self {
        Self {
            start: DocPosition::from_lsp(range.start),
            end: DocPosition::from_lsp(range.end),
        }
    }

    /// Returns `true` if the range spans zero characters.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns `true` if the given position falls within this range.
    pub fn contains(&self, pos: DocPosition) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns `true` if this range overlaps with `other`.
    pub fn overlaps(&self, other: &DocRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Returns `true` if this range spans a single line.
    pub fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Number of lines touched by this range (at least 1 if non-empty).
    pub fn line_span(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            self.end.line - self.start.line + 1
        }
    }
}

impl std::fmt::Display for DocRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

// ---------------------------------------------------------------------------
// Text edit application
// ---------------------------------------------------------------------------

/// A single text edit to be applied to a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleTextEdit {
    pub range: DocRange,
    pub new_text: String,
}

impl SimpleTextEdit {
    pub fn new(range: DocRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    /// Create from an LSP `TextEdit`.
    pub fn from_lsp(edit: &lsp_types::TextEdit) -> Self {
        Self {
            range: DocRange::from_lsp(edit.range),
            new_text: edit.new_text.clone(),
        }
    }

    /// Convert to an LSP `TextEdit`.
    pub fn to_lsp(&self) -> lsp_types::TextEdit {
        lsp_types::TextEdit {
            range: self.range.to_lsp(),
            new_text: self.new_text.clone(),
        }
    }
}

/// Apply a set of non-overlapping text edits to a document.
///
/// Edits are sorted in reverse document order so that earlier edits do not
/// invalidate the positions of later ones.
pub fn apply_edits(text: &str, edits: &[SimpleTextEdit]) -> Result<String, LspError> {
    let mut sorted: Vec<&SimpleTextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| b.range.start.cmp(&a.range.start));

    // Validate no overlaps after sorting.
    for pair in sorted.windows(2) {
        if pair[0].range.overlaps(&pair[1].range) {
            return Err(LspError::InvalidUri(
                "overlapping edits are not supported".into(),
            ));
        }
    }

    let lines: Vec<&str> = text.split('\n').collect();
    let mut result = text.to_string();

    for edit in &sorted {
        let start_offset = line_col_to_offset(&lines, edit.range.start)?;
        let end_offset = line_col_to_offset(&lines, edit.range.end)?;
        result.replace_range(start_offset..end_offset, &edit.new_text);
    }

    Ok(result)
}

/// Convert a (line, col) position to a byte offset in the original text.
fn line_col_to_offset(lines: &[&str], pos: DocPosition) -> Result<usize, LspError> {
    let line = pos.line as usize;
    if line > lines.len() {
        return Err(LspError::InvalidUri(format!(
            "line {} out of range (max {})",
            line,
            lines.len()
        )));
    }
    // Allow pointing one past the last line (for end-of-document edits).
    if line == lines.len() {
        return Ok(lines.iter().map(|l| l.len() + 1).sum::<usize>().saturating_sub(1));
    }
    let col = pos.col as usize;
    if col > lines[line].len() {
        return Err(LspError::InvalidUri(format!(
            "col {} out of range on line {} (max {})",
            col,
            line,
            lines[line].len()
        )));
    }
    let mut offset: usize = 0;
    for l in &lines[..line] {
        offset += l.len() + 1; // +1 for the '\n'
    }
    offset += col;
    Ok(offset)
}

// ---------------------------------------------------------------------------
// Diagnostic aggregation
// ---------------------------------------------------------------------------

/// An owned diagnostic entry tied to a document URI.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticEntry {
    pub uri: String,
    pub range: DocRange,
    pub severity: Severity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

impl std::fmt::Display for DiagnosticEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} ({}): {}",
            self.severity,
            self.uri,
            self.range,
            self.message
        )
    }
}

/// A collection of diagnostics across multiple documents.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticCollection {
    entries: Vec<DiagnosticEntry>,
}

impl DiagnosticCollection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all diagnostics for a given URI.
    pub fn set_for_uri(&mut self, uri: &str, diags: Vec<DiagnosticEntry>) {
        self.entries.retain(|e| e.uri != uri);
        self.entries.extend(diags);
    }

    /// Ingest an LSP `PublishDiagnosticsParams` notification.
    pub fn ingest(&mut self, params: &lsp_types::PublishDiagnosticsParams) {
        let uri = params.uri.as_str().to_string();
        let entries: Vec<DiagnosticEntry> = params
            .diagnostics
            .iter()
            .map(|d| DiagnosticEntry {
                uri: uri.clone(),
                range: DocRange::from_lsp(d.range),
                severity: d
                    .severity
                    .map(Severity::from_lsp)
                    .unwrap_or(Severity::Info),
                message: d.message.clone(),
                source: d.source.clone(),
                code: d.code.as_ref().map(|c| match c {
                    lsp_types::NumberOrString::Number(n) => n.to_string(),
                    lsp_types::NumberOrString::String(s) => s.clone(),
                }),
            })
            .collect();
        self.set_for_uri(&uri, entries);
    }

    /// Clear all diagnostics.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Clear diagnostics for a single URI.
    pub fn clear_uri(&mut self, uri: &str) {
        self.entries.retain(|e| e.uri != uri);
    }

    /// Return all diagnostics.
    pub fn all(&self) -> &[DiagnosticEntry] {
        &self.entries
    }

    /// Return diagnostics for a given URI.
    pub fn for_uri(&self, uri: &str) -> Vec<&DiagnosticEntry> {
        self.entries.iter().filter(|e| e.uri == uri).collect()
    }

    /// Return diagnostics at or above a minimum severity.
    pub fn at_severity(&self, min: Severity) -> Vec<&DiagnosticEntry> {
        self.entries.iter().filter(|e| e.severity.at_least(min)).collect()
    }

    /// Return the total number of stored diagnostics.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if there are no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Count diagnostics of a specific severity.
    pub fn count_severity(&self, sev: Severity) -> usize {
        self.entries.iter().filter(|e| e.severity == sev).count()
    }

    /// Return all unique URIs that have diagnostics.
    pub fn uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.entries.iter().map(|e| e.uri.clone()).collect();
        uris.sort();
        uris.dedup();
        uris
    }

    /// Return diagnostics for a specific line in a URI.
    pub fn for_line(&self, uri: &str, line: u32) -> Vec<&DiagnosticEntry> {
        self.entries
            .iter()
            .filter(|e| e.uri == uri && e.range.start.line <= line && e.range.end.line >= line)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// LSP capability checking
// ---------------------------------------------------------------------------

/// Describes which LSP features a server supports after initialization.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerCapabilityFlags {
    pub completion: bool,
    pub hover: bool,
    pub definition: bool,
    pub references: bool,
    pub rename: bool,
    pub formatting: bool,
    pub range_formatting: bool,
    pub code_action: bool,
    pub document_symbol: bool,
    pub signature_help: bool,
}

impl ServerCapabilityFlags {
    /// Extract capability flags from an `lsp_types::ServerCapabilities`.
    pub fn from_server_capabilities(caps: &lsp_types::ServerCapabilities) -> Self {
        Self {
            completion: caps.completion_provider.is_some(),
            hover: caps.hover_provider.is_some(),
            definition: caps.definition_provider.is_some(),
            references: caps.references_provider.is_some(),
            rename: caps.rename_provider.is_some(),
            formatting: caps.document_formatting_provider.is_some(),
            range_formatting: caps.document_range_formatting_provider.is_some(),
            code_action: caps.code_action_provider.is_some(),
            document_symbol: caps.document_symbol_provider.is_some(),
            signature_help: caps.signature_help_provider.is_some(),
        }
    }

    /// Return a list of feature names that are supported.
    pub fn supported_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();
        if self.completion {
            features.push("completion");
        }
        if self.hover {
            features.push("hover");
        }
        if self.definition {
            features.push("definition");
        }
        if self.references {
            features.push("references");
        }
        if self.rename {
            features.push("rename");
        }
        if self.formatting {
            features.push("formatting");
        }
        if self.range_formatting {
            features.push("range_formatting");
        }
        if self.code_action {
            features.push("code_action");
        }
        if self.document_symbol {
            features.push("document_symbol");
        }
        if self.signature_help {
            features.push("signature_help");
        }
        features
    }

    /// Check whether a named feature is supported.
    pub fn supports(&self, feature: &str) -> bool {
        match feature {
            "completion" => self.completion,
            "hover" => self.hover,
            "definition" => self.definition,
            "references" => self.references,
            "rename" => self.rename,
            "formatting" => self.formatting,
            "range_formatting" => self.range_formatting,
            "code_action" => self.code_action,
            "document_symbol" => self.document_symbol,
            "signature_help" => self.signature_help,
            _ => false,
        }
    }
}

impl std::fmt::Display for ServerCapabilityFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let features = self.supported_features();
        if features.is_empty() {
            write!(f, "(no capabilities)")
        } else {
            write!(f, "{}", features.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// URI validation
// ---------------------------------------------------------------------------

/// Validate that a string looks like a reasonable `file://` URI.
pub fn validate_file_uri(uri: &str) -> Result<(), LspError> {
    if !uri.starts_with("file://") {
        return Err(LspError::InvalidUri(format!(
            "expected file:// scheme, got: {uri}"
        )));
    }
    if uri.len() <= "file://".len() {
        return Err(LspError::InvalidUri("empty path in file URI".into()));
    }
    Ok(())
}

/// Build a `file://` URI from an absolute file path.
pub fn path_to_file_uri(path: &str) -> Result<String, LspError> {
    if !path.starts_with('/') {
        return Err(LspError::InvalidUri(format!(
            "expected absolute path, got: {path}"
        )));
    }
    Ok(format!("file://{path}"))
}

/// Extract the file-system path from a `file://` URI.
pub fn file_uri_to_path(uri: &str) -> Result<String, LspError> {
    validate_file_uri(uri)?;
    Ok(uri["file://".len()..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::LspServerConfig;
    use crate::manager::LspManager;
    use crate::transport::*;

    #[test]
    fn lsp_error_display() {
        let err = LspError::SpawnFailed("not found".into());
        assert!(err.to_string().contains("not found"));

        let err = LspError::NoStdin;
        assert!(err.to_string().contains("stdin"));

        let err = LspError::ServerError {
            code: -32600,
            message: "bad".into(),
        };
        assert!(err.to_string().contains("-32600"));
    }

    #[test]
    fn lsp_server_config_clone() {
        let cfg = LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec!["--stdio".to_string()],
            language_ids: vec!["rust".to_string()],
            root_patterns: vec![".rs".to_string()],
        };
        let cfg2 = cfg.clone();
        assert_eq!(cfg2.command, "rust-analyzer");
        assert_eq!(cfg2.language_ids, vec!["rust"]);
    }

    #[test]
    fn manager_register_and_unregister() {
        let mut mgr = LspManager::new();
        let cfg = LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
            root_patterns: vec![".rs".to_string()],
        };
        mgr.register("rust", cfg);
        assert!(mgr.registered_languages().contains(&"rust".to_string()));
        assert!(mgr.unregister("rust"));
        assert!(!mgr.unregister("rust"));
    }

    #[test]
    fn manager_language_for_file() {
        let mut mgr = LspManager::new();
        mgr.register(
            "rust",
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                language_ids: vec!["rust".to_string()],
                root_patterns: vec![".rs".to_string()],
            },
        );
        mgr.register(
            "python",
            LspServerConfig {
                command: "pylsp".to_string(),
                args: vec![],
                language_ids: vec!["python".to_string()],
                root_patterns: vec![".py".to_string()],
            },
        );
        assert_eq!(mgr.language_for_file("main.rs"), Some("rust".to_string()));
        assert_eq!(mgr.language_for_file("app.py"), Some("python".to_string()));
        assert_eq!(mgr.language_for_file("style.css"), None);
    }

    #[test]
    fn manager_is_active_false_when_not_started() {
        let _mgr = LspManager::new();
        assert!(!_mgr.is_active("rust"));
    }

    #[test]
    fn manager_active_languages_empty() {
        let _mgr = LspManager::new();
        assert!(_mgr.active_languages().is_empty());
    }

    #[test]
    fn manager_default() {
        let _mgr = LspManager::default();
        assert!(_mgr.registered_languages().is_empty());
    }

    #[tokio::test]
    async fn manager_start_no_config() {
        let mut mgr = LspManager::new();
        let result = mgr.start("rust").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LspError::NoConfig(_)));
    }

    #[tokio::test]
    async fn manager_stop_nonexistent_is_ok() {
        let mut mgr = LspManager::new();
        let result = mgr.stop("rust").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn spawn_nonexistent_command_fails() {
        let result = LspClient::spawn_server("nonexistent-lsp-binary-12345", &[]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LspError::SpawnFailed(_)));
    }

    #[test]
    fn transport_encode_decode_notification() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didSave".to_string(),
            params: Some(serde_json::json!({"uri": "file:///test.rs"})),
        };
        let encoded = encode_message(&notif);
        let (msg, _) = try_decode_message(&encoded).unwrap().unwrap();
        assert!(msg.is_notification());
        assert_eq!(msg.method.as_deref(), Some("textDocument/didSave"));
    }

    #[test]
    fn transport_encode_decode_response() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(42),
            result: Some(serde_json::json!({"capabilities": {}})),
            error: None,
        };
        let encoded = encode_message(&resp);
        let (msg, _) = try_decode_message(&encoded).unwrap().unwrap();
        assert!(msg.is_response());
        assert_eq!(msg.id, Some(42));
    }

    #[test]
    fn lsp_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
        let _lsp_err = LspError::from(io_err);
        assert!(_lsp_err.to_string().contains("broken"));
    }

    // -----------------------------------------------------------------------
    // New tests for added utilities
    // -----------------------------------------------------------------------

    #[test]
    fn severity_from_lsp_roundtrip() {
        for &sev in &[Severity::Error, Severity::Warning, Severity::Info, Severity::Hint] {
            assert_eq!(Severity::from_lsp(sev.to_lsp()), sev);
        }
    }

    #[test]
    fn severity_labels_and_symbols() {
        assert_eq!(Severity::Error.label(), "error");
        assert_eq!(Severity::Warning.symbol(), 'W');
        assert_eq!(Severity::Hint.symbol(), 'H');
        assert_eq!(format!("{}", Severity::Info), "info");
    }

    #[test]
    fn severity_at_least() {
        assert!(Severity::Error.at_least(Severity::Warning));
        assert!(Severity::Error.at_least(Severity::Error));
        assert!(!Severity::Hint.at_least(Severity::Warning));
        assert!(Severity::Warning.at_least(Severity::Hint));
    }

    #[test]
    fn doc_position_ordering() {
        let a = DocPosition::new(0, 5);
        let b = DocPosition::new(1, 0);
        let c = DocPosition::new(0, 10);
        assert!(a < b);
        assert!(a < c);
        assert!(b > c);
    }

    #[test]
    fn doc_position_display() {
        let p = DocPosition::new(3, 7);
        assert_eq!(format!("{p}"), "4:8"); // 1-indexed display
    }

    #[test]
    fn doc_position_lsp_roundtrip() {
        let p = DocPosition::new(10, 20);
        let lsp = p.to_lsp();
        assert_eq!(DocPosition::from_lsp(lsp), p);
    }

    #[test]
    fn doc_range_contains_and_overlap() {
        let r = DocRange::new(DocPosition::new(1, 0), DocPosition::new(1, 10));
        assert!(r.contains(DocPosition::new(1, 5)));
        assert!(!r.contains(DocPosition::new(1, 10))); // end is exclusive
        assert!(!r.contains(DocPosition::new(0, 5)));

        let r2 = DocRange::new(DocPosition::new(1, 5), DocPosition::new(2, 0));
        assert!(r.overlaps(&r2));
        assert!(r2.overlaps(&r));

        let r3 = DocRange::new(DocPosition::new(3, 0), DocPosition::new(3, 5));
        assert!(!r.overlaps(&r3));
    }

    #[test]
    fn doc_range_properties() {
        let empty = DocRange::new(DocPosition::new(1, 5), DocPosition::new(1, 5));
        assert!(empty.is_empty());
        assert_eq!(empty.line_span(), 0);

        let single = DocRange::new(DocPosition::new(1, 0), DocPosition::new(1, 10));
        assert!(single.is_single_line());
        assert_eq!(single.line_span(), 1);

        let multi = DocRange::new(DocPosition::new(1, 0), DocPosition::new(3, 5));
        assert!(!multi.is_single_line());
        assert_eq!(multi.line_span(), 3);
    }

    #[test]
    fn simple_text_edit_lsp_roundtrip() {
        let edit = SimpleTextEdit::new(
            DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 5)),
            "hello",
        );
        let lsp = edit.to_lsp();
        let back = SimpleTextEdit::from_lsp(&lsp);
        assert_eq!(edit, back);
    }

    #[test]
    fn apply_edits_single() {
        let text = "hello world";
        let edit = SimpleTextEdit::new(
            DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 5)),
            "goodbye",
        );
        let result = apply_edits(text, &[edit]).unwrap();
        assert_eq!(result, "goodbye world");
    }

    #[test]
    fn apply_edits_multiple_non_overlapping() {
        let text = "aaa bbb ccc";
        let e1 = SimpleTextEdit::new(
            DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 3)),
            "AAA",
        );
        let e2 = SimpleTextEdit::new(
            DocRange::new(DocPosition::new(0, 8), DocPosition::new(0, 11)),
            "CCC",
        );
        let result = apply_edits(text, &[e1, e2]).unwrap();
        assert_eq!(result, "AAA bbb CCC");
    }

    #[test]
    fn apply_edits_multiline() {
        let text = "line0\nline1\nline2";
        let edit = SimpleTextEdit::new(
            DocRange::new(DocPosition::new(1, 0), DocPosition::new(1, 5)),
            "REPLACED",
        );
        let result = apply_edits(text, &[edit]).unwrap();
        assert_eq!(result, "line0\nREPLACED\nline2");
    }

    #[test]
    fn diagnostic_collection_basic() {
        let mut coll = DiagnosticCollection::new();
        assert!(coll.is_empty());

        let entry = DiagnosticEntry {
            uri: "file:///a.rs".into(),
            range: DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 5)),
            severity: Severity::Error,
            message: "boom".into(),
            source: Some("rustc".into()),
            code: Some("E0001".into()),
        };
        coll.set_for_uri("file:///a.rs", vec![entry.clone()]);
        assert_eq!(coll.len(), 1);
        assert_eq!(coll.count_severity(Severity::Error), 1);
        assert_eq!(coll.count_severity(Severity::Warning), 0);
        assert_eq!(coll.for_uri("file:///a.rs").len(), 1);
        assert_eq!(coll.for_line("file:///a.rs", 0).len(), 1);
        assert_eq!(coll.for_line("file:///a.rs", 1).len(), 0);
        assert_eq!(coll.uris(), vec!["file:///a.rs".to_string()]);

        coll.clear_uri("file:///a.rs");
        assert!(coll.is_empty());
    }

    #[test]
    fn diagnostic_collection_at_severity() {
        let mut coll = DiagnosticCollection::new();
        let mk = |sev: Severity| DiagnosticEntry {
            uri: "file:///a.rs".into(),
            range: DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 1)),
            severity: sev,
            message: format!("{sev}"),
            source: None,
            code: None,
        };
        coll.set_for_uri(
            "file:///a.rs",
            vec![mk(Severity::Error), mk(Severity::Warning), mk(Severity::Hint)],
        );
        assert_eq!(coll.at_severity(Severity::Warning).len(), 2); // error + warning
        assert_eq!(coll.at_severity(Severity::Hint).len(), 3);    // all
        assert_eq!(coll.at_severity(Severity::Error).len(), 1);   // error only
    }

    #[test]
    fn diagnostic_entry_display() {
        let entry = DiagnosticEntry {
            uri: "file:///a.rs".into(),
            range: DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 5)),
            severity: Severity::Warning,
            message: "unused".into(),
            source: None,
            code: None,
        };
        let s = format!("{entry}");
        assert!(s.contains("warning"));
        assert!(s.contains("unused"));
    }

    #[test]
    fn server_capability_flags_default_empty() {
        let flags = ServerCapabilityFlags::default();
        assert!(flags.supported_features().is_empty());
        assert_eq!(format!("{flags}"), "(no capabilities)");
        assert!(!flags.supports("completion"));
    }

    #[test]
    fn server_capability_flags_supports() {
        let flags = ServerCapabilityFlags {
            completion: true,
            hover: true,
            ..Default::default()
        };
        assert!(flags.supports("completion"));
        assert!(flags.supports("hover"));
        assert!(!flags.supports("rename"));
        assert!(!flags.supports("unknown_feature"));
        let features = flags.supported_features();
        assert_eq!(features.len(), 2);
        assert!(format!("{flags}").contains("completion"));
    }

    #[test]
    fn validate_file_uri_valid() {
        assert!(validate_file_uri("file:///home/user/file.rs").is_ok());
    }

    #[test]
    fn validate_file_uri_invalid() {
        assert!(validate_file_uri("http://example.com").is_err());
        assert!(validate_file_uri("file://").is_err());
    }

    #[test]
    fn path_to_file_uri_and_back() {
        let path = "/home/user/project/main.rs";
        let uri = path_to_file_uri(path).unwrap();
        assert_eq!(uri, "file:///home/user/project/main.rs");
        let back = file_uri_to_path(&uri).unwrap();
        assert_eq!(back, path);
    }

    #[test]
    fn path_to_file_uri_relative_fails() {
        assert!(path_to_file_uri("relative/path.rs").is_err());
    }
}
