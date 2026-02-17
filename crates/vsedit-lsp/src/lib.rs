//! LSP client integration for language server communication.
//!
//! Provides a JSON-RPC transport, an [`LspClient`](client::LspClient) for
//! communicating with a single language server, and an
//! [`LspManager`](manager::LspManager) that manages multiple servers (one per
//! language).

pub mod client;
pub mod manager;
pub mod transport;

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// LSP server registry
// ---------------------------------------------------------------------------

/// Metadata for a single language server.
#[derive(Debug, Clone)]
pub struct LspServerInfo {
    pub language_id: String,
    pub server_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub capabilities: ServerCapabilityFlags,
    pub is_running: bool,
}

impl LspServerInfo {
    pub fn new(
        language_id: impl Into<String>,
        server_name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            server_name: server_name.into(),
            command: command.into(),
            args,
            capabilities: ServerCapabilityFlags::default(),
            is_running: false,
        }
    }

    /// Mark this server as running.
    pub fn mark_running(&mut self) {
        self.is_running = true;
    }

    /// Mark this server as stopped.
    pub fn mark_stopped(&mut self) {
        self.is_running = false;
    }

    /// Check whether this server supports a named feature.
    pub fn supports_feature(&self, feature: &str) -> bool {
        self.capabilities.supports(feature)
    }
}

/// Registry that tracks multiple language servers.
#[derive(Debug, Clone, Default)]
pub struct LspServerRegistry {
    servers: Vec<LspServerInfo>,
}

impl LspServerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a language server. Replaces any existing entry for the same
    /// `language_id`.
    pub fn register(&mut self, info: LspServerInfo) {
        self.unregister(&info.language_id);
        self.servers.push(info);
    }

    /// Remove the server registered for `language_id`. Returns `true` if an
    /// entry was removed.
    pub fn unregister(&mut self, language_id: &str) -> bool {
        let before = self.servers.len();
        self.servers.retain(|s| s.language_id != language_id);
        self.servers.len() < before
    }

    /// Look up a server by language id.
    pub fn get(&self, language_id: &str) -> Option<&LspServerInfo> {
        self.servers.iter().find(|s| s.language_id == language_id)
    }

    /// Look up a server mutably by language id.
    pub fn get_mut(&mut self, language_id: &str) -> Option<&mut LspServerInfo> {
        self.servers.iter_mut().find(|s| s.language_id == language_id)
    }

    /// Return references to all currently running servers.
    pub fn running_servers(&self) -> Vec<&LspServerInfo> {
        self.servers.iter().filter(|s| s.is_running).collect()
    }

    /// Total number of registered servers.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Return the language ids of all registered servers.
    pub fn languages(&self) -> Vec<&str> {
        self.servers.iter().map(|s| s.language_id.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Negotiated LSP capabilities
// ---------------------------------------------------------------------------

/// Describes the text document synchronisation kind negotiated with the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextDocSyncKind {
    #[default]
    None,
    Full,
    Incremental,
}

impl fmt::Display for TextDocSyncKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextDocSyncKind::None => f.write_str("none"),
            TextDocSyncKind::Full => f.write_str("full"),
            TextDocSyncKind::Incremental => f.write_str("incremental"),
        }
    }
}

/// Negotiated capabilities extracted from an LSP `InitializeResult`.
#[derive(Debug, Clone, Default)]
pub struct LspCapabilities {
    pub text_document_sync: TextDocSyncKind,
    pub flags: ServerCapabilityFlags,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

impl LspCapabilities {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from an `InitializeResult` returned by the server.
    pub fn from_initialize_result(result: &lsp_types::InitializeResult) -> Self {
        let flags = ServerCapabilityFlags::from_server_capabilities(&result.capabilities);

        let text_document_sync = result
            .capabilities
            .text_document_sync
            .as_ref()
            .map(|sync| match sync {
                lsp_types::TextDocumentSyncCapability::Kind(k) => match *k {
                    lsp_types::TextDocumentSyncKind::NONE => TextDocSyncKind::None,
                    lsp_types::TextDocumentSyncKind::FULL => TextDocSyncKind::Full,
                    lsp_types::TextDocumentSyncKind::INCREMENTAL => TextDocSyncKind::Incremental,
                    _ => TextDocSyncKind::None,
                },
                lsp_types::TextDocumentSyncCapability::Options(opts) => {
                    opts.change.map_or(TextDocSyncKind::None, |k| match k {
                        lsp_types::TextDocumentSyncKind::NONE => TextDocSyncKind::None,
                        lsp_types::TextDocumentSyncKind::FULL => TextDocSyncKind::Full,
                        lsp_types::TextDocumentSyncKind::INCREMENTAL => {
                            TextDocSyncKind::Incremental
                        }
                        _ => TextDocSyncKind::None,
                    })
                }
            })
            .unwrap_or(TextDocSyncKind::None);

        let (server_name, server_version) = result
            .server_info
            .as_ref()
            .map(|info| (Some(info.name.clone()), info.version.clone()))
            .unwrap_or((None, None));

        Self {
            text_document_sync,
            flags,
            server_name,
            server_version,
        }
    }

    /// Return a human-readable summary of the negotiated capabilities.
    pub fn summary(&self) -> String {
        let name = self
            .server_name
            .as_deref()
            .unwrap_or("unknown server");
        let ver = self
            .server_version
            .as_deref()
            .unwrap_or("?");
        let features = self.flags.supported_features();
        let feat_str = if features.is_empty() {
            "none".to_string()
        } else {
            features.join(", ")
        };
        format!("{name} v{ver} | sync={} | features: {feat_str}", self.text_document_sync)
    }
}

impl fmt::Display for LspCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

// ---------------------------------------------------------------------------
// Initialize params builder
// ---------------------------------------------------------------------------

/// Build an `lsp_types::InitializeParams` from workspace settings.
///
/// The `workspace_root` must be an absolute path. It is converted to a
/// `file://` URI via [`path_to_file_uri`].
pub fn lsp_initialize_params(
    workspace_root: &str,
    client_name: &str,
    client_version: &str,
) -> Result<lsp_types::InitializeParams, LspError> {
    let root_uri = path_to_file_uri(workspace_root)?;

    #[allow(deprecated)] // root_path / root_uri are deprecated but widely used
    Ok(lsp_types::InitializeParams {
        process_id: Some(std::process::id()),
        root_path: Some(workspace_root.to_string()),
        root_uri: Some(
            root_uri
                .parse::<lsp_types::Uri>()
                .map_err(|e| LspError::InvalidUri(e.to_string()))?,
        ),
        initialization_options: None,
        capabilities: lsp_types::ClientCapabilities {
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                synchronization: Some(lsp_types::TextDocumentSyncClientCapabilities {
                    dynamic_registration: Some(false),
                    will_save: Some(false),
                    will_save_wait_until: Some(false),
                    did_save: Some(true),
                }),
                completion: Some(lsp_types::CompletionClientCapabilities {
                    dynamic_registration: Some(false),
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        snippet_support: Some(false),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                hover: Some(lsp_types::HoverClientCapabilities {
                    dynamic_registration: Some(false),
                    content_format: Some(vec![lsp_types::MarkupKind::PlainText]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        trace: Some(lsp_types::TraceValue::Off),
        workspace_folders: None,
        client_info: Some(lsp_types::ClientInfo {
            name: client_name.to_string(),
            version: Some(client_version.to_string()),
        }),
        locale: None,
        work_done_progress_params: Default::default(),
    })
}

// ---------------------------------------------------------------------------
// Diagnostic index (file-keyed)
// ---------------------------------------------------------------------------

/// A file-keyed index of diagnostics for fast lookup by URI.
///
/// Unlike [`DiagnosticCollection`] which stores a flat list,
/// `DiagnosticIndex` maintains a `HashMap` keyed by file URI for O(1)
/// per-file access.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticIndex {
    index: HashMap<String, Vec<DiagnosticEntry>>,
}

impl DiagnosticIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert diagnostics for a file, replacing any previous entries.
    pub fn set_file(&mut self, uri: impl Into<String>, diags: Vec<DiagnosticEntry>) {
        let uri = uri.into();
        if diags.is_empty() {
            self.index.remove(&uri);
        } else {
            self.index.insert(uri, diags);
        }
    }

    /// Get diagnostics for a single file.
    pub fn get_for_file(&self, uri: &str) -> &[DiagnosticEntry] {
        self.index.get(uri).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Number of files that have at least one diagnostic.
    pub fn file_count(&self) -> usize {
        self.index.len()
    }

    /// Total number of diagnostics across all files.
    pub fn total_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Return URIs of files that have at least one error-severity diagnostic.
    pub fn files_with_errors(&self) -> Vec<&str> {
        self.index
            .iter()
            .filter(|(_, diags)| diags.iter().any(|d| d.severity == Severity::Error))
            .map(|(uri, _)| uri.as_str())
            .collect()
    }

    /// Remove all diagnostics for a file.
    pub fn clear_file(&mut self, uri: &str) {
        self.index.remove(uri);
    }

    /// Remove all diagnostics.
    pub fn clear_all(&mut self) {
        self.index.clear();
    }

    /// Return all file URIs that have diagnostics, sorted.
    pub fn file_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.index.keys().map(|s| s.as_str()).collect();
        uris.sort_unstable();
        uris
    }

    /// Count diagnostics of a given severity across all files.
    pub fn count_by_severity(&self, sev: Severity) -> usize {
        self.index
            .values()
            .flat_map(|v| v.iter())
            .filter(|d| d.severity == sev)
            .count()
    }
}

impl From<DiagnosticCollection> for DiagnosticIndex {
    fn from(collection: DiagnosticCollection) -> Self {
        let mut idx = DiagnosticIndex::new();
        for entry in collection.all() {
            idx.index
                .entry(entry.uri.clone())
                .or_default()
                .push(entry.clone());
        }
        idx
    }
}

impl fmt::Display for DiagnosticIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} diagnostic(s) across {} file(s)",
            self.total_count(),
            self.file_count()
        )
    }
}

// ---------------------------------------------------------------------------
// Completion item entry
// ---------------------------------------------------------------------------

/// The kind of a completion item, mirroring common LSP completion kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Keyword,
    Snippet,
    File,
    Constant,
    Enum,
    EnumMember,
    Struct,
    TypeParameter,
    Other,
}

impl CompletionKind {
    /// Convert from an `lsp_types::CompletionItemKind`.
    pub fn from_lsp(kind: lsp_types::CompletionItemKind) -> Self {
        match kind {
            lsp_types::CompletionItemKind::TEXT => Self::Text,
            lsp_types::CompletionItemKind::METHOD => Self::Method,
            lsp_types::CompletionItemKind::FUNCTION => Self::Function,
            lsp_types::CompletionItemKind::CONSTRUCTOR => Self::Constructor,
            lsp_types::CompletionItemKind::FIELD => Self::Field,
            lsp_types::CompletionItemKind::VARIABLE => Self::Variable,
            lsp_types::CompletionItemKind::CLASS => Self::Class,
            lsp_types::CompletionItemKind::INTERFACE => Self::Interface,
            lsp_types::CompletionItemKind::MODULE => Self::Module,
            lsp_types::CompletionItemKind::PROPERTY => Self::Property,
            lsp_types::CompletionItemKind::KEYWORD => Self::Keyword,
            lsp_types::CompletionItemKind::SNIPPET => Self::Snippet,
            lsp_types::CompletionItemKind::FILE => Self::File,
            lsp_types::CompletionItemKind::CONSTANT => Self::Constant,
            lsp_types::CompletionItemKind::ENUM => Self::Enum,
            lsp_types::CompletionItemKind::ENUM_MEMBER => Self::EnumMember,
            lsp_types::CompletionItemKind::STRUCT => Self::Struct,
            lsp_types::CompletionItemKind::TYPE_PARAMETER => Self::TypeParameter,
            _ => Self::Other,
        }
    }

    /// Short symbol for display in menus.
    pub fn symbol(self) -> char {
        match self {
            Self::Text => 'T',
            Self::Method | Self::Function => 'f',
            Self::Constructor => 'C',
            Self::Field | Self::Property => 'p',
            Self::Variable => 'v',
            Self::Class | Self::Struct | Self::Interface => 'S',
            Self::Module => 'M',
            Self::Keyword => 'K',
            Self::Snippet => 's',
            Self::File => 'F',
            Self::Constant => 'c',
            Self::Enum | Self::EnumMember => 'E',
            Self::TypeParameter => 't',
            Self::Other => '?',
        }
    }
}

impl fmt::Display for CompletionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Method => "method",
            Self::Function => "function",
            Self::Constructor => "constructor",
            Self::Field => "field",
            Self::Variable => "variable",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Module => "module",
            Self::Property => "property",
            Self::Keyword => "keyword",
            Self::Snippet => "snippet",
            Self::File => "file",
            Self::Constant => "constant",
            Self::Enum => "enum",
            Self::EnumMember => "enum member",
            Self::Struct => "struct",
            Self::TypeParameter => "type parameter",
            Self::Other => "other",
        })
    }
}

/// A simplified completion item for display in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItemEntry {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub sort_text: Option<String>,
    pub insert_text: Option<String>,
}

impl CompletionItemEntry {
    pub fn new(label: impl Into<String>, kind: CompletionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            sort_text: None,
            insert_text: None,
        }
    }

    /// The text to actually insert; falls back to `label`.
    pub fn text_to_insert(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }

    /// The key used for sorting; falls back to `label`.
    pub fn sort_key(&self) -> &str {
        self.sort_text.as_deref().unwrap_or(&self.label)
    }

    /// Case-insensitive prefix match against a typed prefix.
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        let prefix_lower = prefix.to_lowercase();
        self.label.to_lowercase().starts_with(&prefix_lower)
            || self
                .insert_text
                .as_deref()
                .map(|t| t.to_lowercase().starts_with(&prefix_lower))
                .unwrap_or(false)
    }

    /// Fuzzy match: every character of `query` appears in order in the label.
    pub fn matches_fuzzy(&self, query: &str) -> bool {
        let mut label_chars = self.label.chars().flat_map(|c| c.to_lowercase());
        for qc in query.chars().flat_map(|c| c.to_lowercase()) {
            if !label_chars.any(|lc| lc == qc) {
                return false;
            }
        }
        true
    }
}

impl From<&lsp_types::CompletionItem> for CompletionItemEntry {
    fn from(item: &lsp_types::CompletionItem) -> Self {
        Self {
            label: item.label.clone(),
            kind: item
                .kind
                .map(CompletionKind::from_lsp)
                .unwrap_or(CompletionKind::Text),
            detail: item.detail.clone(),
            sort_text: item.sort_text.clone(),
            insert_text: item.insert_text.clone(),
        }
    }
}

impl fmt::Display for CompletionItemEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.symbol(), self.label)?;
        if let Some(ref detail) = self.detail {
            write!(f, " — {detail}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Completion list
// ---------------------------------------------------------------------------

/// A list of completion items, possibly incomplete (more results available).
#[derive(Debug, Clone, Default)]
pub struct CompletionList {
    pub items: Vec<CompletionItemEntry>,
    pub is_incomplete: bool,
}

impl CompletionList {
    pub fn new(items: Vec<CompletionItemEntry>, is_incomplete: bool) -> Self {
        Self {
            items,
            is_incomplete,
        }
    }

    /// Filter items by a typed prefix (case-insensitive).
    pub fn filter_prefix(&self, prefix: &str) -> CompletionList {
        CompletionList {
            items: self
                .items
                .iter()
                .filter(|i| i.matches_prefix(prefix))
                .cloned()
                .collect(),
            is_incomplete: self.is_incomplete,
        }
    }

    /// Filter items by fuzzy match.
    pub fn filter_fuzzy(&self, query: &str) -> CompletionList {
        CompletionList {
            items: self
                .items
                .iter()
                .filter(|i| i.matches_fuzzy(query))
                .cloned()
                .collect(),
            is_incomplete: self.is_incomplete,
        }
    }

    /// Return a new list sorted by sort key.
    pub fn sorted(&self) -> CompletionList {
        let mut items = self.items.clone();
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        CompletionList {
            items,
            is_incomplete: self.is_incomplete,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True if there are no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Filter items by completion kind.
    pub fn filter_kind(&self, kind: CompletionKind) -> CompletionList {
        CompletionList {
            items: self
                .items
                .iter()
                .filter(|i| i.kind == kind)
                .cloned()
                .collect(),
            is_incomplete: self.is_incomplete,
        }
    }
}

impl From<lsp_types::CompletionResponse> for CompletionList {
    fn from(resp: lsp_types::CompletionResponse) -> Self {
        match resp {
            lsp_types::CompletionResponse::Array(items) => CompletionList {
                items: items.iter().map(CompletionItemEntry::from).collect(),
                is_incomplete: false,
            },
            lsp_types::CompletionResponse::List(list) => CompletionList {
                items: list.items.iter().map(CompletionItemEntry::from).collect(),
                is_incomplete: list.is_incomplete,
            },
        }
    }
}

impl fmt::Display for CompletionList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} item(s)", self.items.len())?;
        if self.is_incomplete {
            f.write_str(" (incomplete)")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Diagnostic filtering
// ---------------------------------------------------------------------------

/// Severity levels for LSP diagnostics, ordered from most to least severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverityLevel {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverityLevel {
    /// Returns a numeric rank where lower values are more severe.
    pub fn severity_rank(s: &DiagnosticSeverityLevel) -> u8 {
        match s {
            DiagnosticSeverityLevel::Error => 0,
            DiagnosticSeverityLevel::Warning => 1,
            DiagnosticSeverityLevel::Information => 2,
            DiagnosticSeverityLevel::Hint => 3,
        }
    }
}

impl Ord for DiagnosticSeverityLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // More severe (lower rank) sorts first.
        DiagnosticSeverityLevel::severity_rank(self)
            .cmp(&DiagnosticSeverityLevel::severity_rank(other))
    }
}

impl PartialOrd for DiagnosticSeverityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for DiagnosticSeverityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverityLevel::Error => f.write_str("error"),
            DiagnosticSeverityLevel::Warning => f.write_str("warning"),
            DiagnosticSeverityLevel::Information => f.write_str("information"),
            DiagnosticSeverityLevel::Hint => f.write_str("hint"),
        }
    }
}

/// Filters diagnostics by severity, code, and source.
#[derive(Debug, Clone)]
pub struct LspDiagnosticFilter {
    pub min_severity: DiagnosticSeverityLevel,
    pub ignored_codes: Vec<String>,
    pub source_filter: Option<String>,
}

impl LspDiagnosticFilter {
    pub fn new(min: DiagnosticSeverityLevel) -> Self {
        Self {
            min_severity: min,
            ignored_codes: Vec::new(),
            source_filter: None,
        }
    }

    pub fn ignore_code(&mut self, code: &str) {
        self.ignored_codes.push(code.to_string());
    }

    pub fn set_source_filter(&mut self, source: &str) {
        self.source_filter = Some(source.to_string());
    }

    /// Returns `true` when a diagnostic with the given attributes should be
    /// displayed according to this filter.
    pub fn should_show(
        &self,
        severity: &DiagnosticSeverityLevel,
        code: Option<&str>,
        source: Option<&str>,
    ) -> bool {
        // Reject if less severe than the minimum.
        if DiagnosticSeverityLevel::severity_rank(severity)
            > DiagnosticSeverityLevel::severity_rank(&self.min_severity)
        {
            return false;
        }
        // Reject if the code is on the ignore list.
        if let Some(c) = code {
            if self.ignored_codes.iter().any(|ic| ic == c) {
                return false;
            }
        }
        // Reject if a source filter is set and the source doesn't match.
        if let Some(ref sf) = self.source_filter {
            match source {
                Some(s) if s == sf => {}
                _ => return false,
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Code-action / quick-fix helpers
// ---------------------------------------------------------------------------

/// A lightweight representation of an LSP code-action quick-fix.
#[derive(Debug, Clone)]
pub struct LspCodeActionQuickFix {
    pub title: String,
    pub kind: String,
    pub edit_count: usize,
    pub is_preferred: bool,
}

impl LspCodeActionQuickFix {
    pub fn new(title: &str, kind: &str) -> Self {
        Self {
            title: title.to_string(),
            kind: kind.to_string(),
            edit_count: 0,
            is_preferred: false,
        }
    }

    pub fn with_preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }

    pub fn with_edits(mut self, count: usize) -> Self {
        self.edit_count = count;
        self
    }

    /// Returns `true` when the action kind starts with `"quickfix"`.
    pub fn is_quick_fix(&self) -> bool {
        self.kind.starts_with("quickfix")
    }

    pub fn summary(&self) -> String {
        let preferred = if self.is_preferred { " [preferred]" } else { "" };
        format!(
            "{} ({}, {} edit(s)){}",
            self.title, self.kind, self.edit_count, preferred
        )
    }
}

impl fmt::Display for LspCodeActionQuickFix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

// ---------------------------------------------------------------------------
// Workspace symbol search
// ---------------------------------------------------------------------------

/// A single symbol found via workspace-symbol search.
#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: String,
    pub location: String,
    pub container: Option<String>,
}

impl fmt::Display for WorkspaceSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] @ {}", self.name, self.kind, self.location)?;
        if let Some(ref c) = self.container {
            write!(f, " (in {})", c)?;
        }
        Ok(())
    }
}

/// Collects workspace-symbol search results.
#[derive(Debug, Clone)]
pub struct LspWorkspaceSymbolSearch {
    pub query: String,
    pub results: Vec<WorkspaceSymbol>,
}

impl LspWorkspaceSymbolSearch {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, symbol: WorkspaceSymbol) {
        self.results.push(symbol);
    }

    pub fn filter_by_kind<'a>(&'a self, kind: &str) -> Vec<&'a WorkspaceSymbol> {
        self.results.iter().filter(|s| s.kind == kind).collect()
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Progress tracking
// ---------------------------------------------------------------------------

/// State of a single progress token.
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub title: String,
    pub message: Option<String>,
    pub percentage: Option<u32>,
    pub done: bool,
}

/// Tracks `$/progress` notifications from one or more language servers.
#[derive(Debug, Clone)]
pub struct LspProgressTracker {
    pub tokens: HashMap<String, ProgressState>,
}

impl LspProgressTracker {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    pub fn begin(&mut self, token: &str, title: &str) {
        self.tokens.insert(
            token.to_string(),
            ProgressState {
                title: title.to_string(),
                message: None,
                percentage: None,
                done: false,
            },
        );
    }

    pub fn report(&mut self, token: &str, message: Option<&str>, percentage: Option<u32>) {
        if let Some(state) = self.tokens.get_mut(token) {
            state.message = message.map(|m| m.to_string());
            state.percentage = percentage;
        }
    }

    pub fn end(&mut self, token: &str) {
        if let Some(state) = self.tokens.get_mut(token) {
            state.done = true;
            state.message = None;
            state.percentage = Some(100);
        }
    }

    /// Number of progress tokens that are still active (not done).
    pub fn active_count(&self) -> usize {
        self.tokens.values().filter(|s| !s.done).count()
    }

    pub fn is_done(&self, token: &str) -> bool {
        self.tokens.get(token).map_or(true, |s| s.done)
    }

    /// Renders a simple ASCII progress bar, e.g. `[=====>    ]`.
    pub fn render_progress_bar(pct: u32, width: usize) -> String {
        let pct = pct.min(100) as usize;
        let filled = width * pct / 100;
        let empty = width - filled;
        let arrow = if filled > 0 && empty > 0 { ">" } else { "" };
        let fill = "=".repeat(if arrow.is_empty() { filled } else { filled.saturating_sub(1) });
        let space = " ".repeat(if arrow.is_empty() { empty } else { empty.saturating_sub(0) });
        format!("[{}{}{}]", fill, arrow, space)
    }
}

impl Default for LspProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LspWorkspaceFoldersSync – synchronizes workspace folders with LSP server
// ---------------------------------------------------------------------------

/// Represents a workspace folder tracked by the LSP client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedWorkspaceFolder {
    pub uri: String,
    pub name: String,
    pub added_at: u64,
}

/// Event describing a change to the set of workspace folders.
#[derive(Debug, Clone)]
pub enum WorkspaceFolderChange {
    Added(TrackedWorkspaceFolder),
    Removed(String),
}

/// Synchronizes workspace folders between the editor and the LSP server.
#[derive(Debug)]
pub struct LspWorkspaceFoldersSync {
    folders: Vec<TrackedWorkspaceFolder>,
    pending_changes: Vec<WorkspaceFolderChange>,
    sync_count: usize,
}

impl LspWorkspaceFoldersSync {
    pub fn new() -> Self {
        Self {
            folders: Vec::new(),
            pending_changes: Vec::new(),
            sync_count: 0,
        }
    }

    /// Add a workspace folder, queuing a notification.
    pub fn add_folder(&mut self, uri: impl Into<String>, name: impl Into<String>, timestamp: u64) {
        let folder = TrackedWorkspaceFolder {
            uri: uri.into(),
            name: name.into(),
            added_at: timestamp,
        };
        if !self.folders.iter().any(|f| f.uri == folder.uri) {
            self.pending_changes.push(WorkspaceFolderChange::Added(folder.clone()));
            self.folders.push(folder);
        }
    }

    /// Remove a workspace folder by URI.
    pub fn remove_folder(&mut self, uri: &str) -> bool {
        let before = self.folders.len();
        self.folders.retain(|f| f.uri != uri);
        if self.folders.len() < before {
            self.pending_changes.push(WorkspaceFolderChange::Removed(uri.to_string()));
            true
        } else {
            false
        }
    }

    /// Drain pending changes (to send as `workspace/didChangeWorkspaceFolders`).
    pub fn drain_pending(&mut self) -> Vec<WorkspaceFolderChange> {
        self.sync_count += 1;
        std::mem::take(&mut self.pending_changes)
    }

    /// Whether there are unsent changes.
    pub fn has_pending(&self) -> bool {
        !self.pending_changes.is_empty()
    }

    pub fn folder_count(&self) -> usize { self.folders.len() }
    pub fn folders(&self) -> &[TrackedWorkspaceFolder] { &self.folders }
    pub fn sync_count(&self) -> usize { self.sync_count }

    /// Find a folder by URI.
    pub fn find(&self, uri: &str) -> Option<&TrackedWorkspaceFolder> {
        self.folders.iter().find(|f| f.uri == uri)
    }

    /// Build the LSP-compatible `added` / `removed` arrays from pending changes.
    pub fn build_change_event(&self) -> (Vec<String>, Vec<String>) {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        for change in &self.pending_changes {
            match change {
                WorkspaceFolderChange::Added(f) => added.push(f.uri.clone()),
                WorkspaceFolderChange::Removed(uri) => removed.push(uri.clone()),
            }
        }
        (added, removed)
    }
}

// ---------------------------------------------------------------------------
// LspFileOperationHandler – handles LSP file create/rename/delete operations
// ---------------------------------------------------------------------------

/// Kind of file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperationKind {
    Create,
    Rename,
    Delete,
}

/// A single file operation.
#[derive(Debug, Clone)]
pub struct FileOperation {
    pub kind: FileOperationKind,
    pub uri: String,
    pub new_uri: Option<String>,
}

/// Collects and processes file operations for LSP notification.
#[derive(Debug)]
pub struct LspFileOperationHandler {
    operations: Vec<FileOperation>,
    create_count: usize,
    rename_count: usize,
    delete_count: usize,
}

impl LspFileOperationHandler {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            create_count: 0,
            rename_count: 0,
            delete_count: 0,
        }
    }

    /// Record a file creation.
    pub fn record_create(&mut self, uri: impl Into<String>) {
        self.create_count += 1;
        self.operations.push(FileOperation {
            kind: FileOperationKind::Create,
            uri: uri.into(),
            new_uri: None,
        });
    }

    /// Record a file rename.
    pub fn record_rename(&mut self, old_uri: impl Into<String>, new_uri: impl Into<String>) {
        self.rename_count += 1;
        self.operations.push(FileOperation {
            kind: FileOperationKind::Rename,
            uri: old_uri.into(),
            new_uri: Some(new_uri.into()),
        });
    }

    /// Record a file deletion.
    pub fn record_delete(&mut self, uri: impl Into<String>) {
        self.delete_count += 1;
        self.operations.push(FileOperation {
            kind: FileOperationKind::Delete,
            uri: uri.into(),
            new_uri: None,
        });
    }

    /// Drain all recorded operations.
    pub fn drain(&mut self) -> Vec<FileOperation> {
        std::mem::take(&mut self.operations)
    }

    /// Filter operations by kind.
    pub fn filter_by_kind(&self, kind: FileOperationKind) -> Vec<&FileOperation> {
        self.operations.iter().filter(|op| op.kind == kind).collect()
    }

    /// Check if a URI was affected by any operation.
    pub fn is_affected(&self, uri: &str) -> bool {
        self.operations.iter().any(|op| {
            op.uri == uri || op.new_uri.as_deref() == Some(uri)
        })
    }

    pub fn total_count(&self) -> usize { self.operations.len() }
    pub fn create_count(&self) -> usize { self.create_count }
    pub fn rename_count(&self) -> usize { self.rename_count }
    pub fn delete_count(&self) -> usize { self.delete_count }
    pub fn operations(&self) -> &[FileOperation] { &self.operations }

    /// Check if any operations are recorded.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
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

    // -----------------------------------------------------------------------
    // LspServerInfo / LspServerRegistry tests
    // -----------------------------------------------------------------------

    #[test]
    fn server_info_new_and_running() {
        let mut info = LspServerInfo::new("rust", "rust-analyzer", "rust-analyzer", vec![]);
        assert!(!info.is_running);
        info.mark_running();
        assert!(info.is_running);
        info.mark_stopped();
        assert!(!info.is_running);
    }

    #[test]
    fn server_info_supports_feature() {
        let mut info = LspServerInfo::new("rust", "rust-analyzer", "rust-analyzer", vec![]);
        info.capabilities.completion = true;
        assert!(info.supports_feature("completion"));
        assert!(!info.supports_feature("rename"));
        assert!(!info.supports_feature("unknown_feature"));
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = LspServerRegistry::new();
        assert_eq!(reg.server_count(), 0);

        reg.register(LspServerInfo::new("rust", "ra", "rust-analyzer", vec![]));
        reg.register(LspServerInfo::new(
            "python",
            "pyright",
            "pyright-langserver",
            vec!["--stdio".into()],
        ));

        assert_eq!(reg.server_count(), 2);
        assert!(reg.get("rust").is_some());
        assert!(reg.get("python").is_some());
        assert!(reg.get("go").is_none());
    }

    #[test]
    fn registry_unregister() {
        let mut reg = LspServerRegistry::new();
        reg.register(LspServerInfo::new("rust", "ra", "rust-analyzer", vec![]));
        assert!(reg.unregister("rust"));
        assert!(!reg.unregister("rust")); // already removed
        assert_eq!(reg.server_count(), 0);
    }

    #[test]
    fn registry_running_servers() {
        let mut reg = LspServerRegistry::new();
        reg.register(LspServerInfo::new("rust", "ra", "rust-analyzer", vec![]));
        reg.register(LspServerInfo::new("python", "pyright", "pyright", vec![]));

        assert!(reg.running_servers().is_empty());

        reg.get_mut("rust").unwrap().mark_running();
        let running = reg.running_servers();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].language_id, "rust");
    }

    #[test]
    fn registry_languages() {
        let mut reg = LspServerRegistry::new();
        reg.register(LspServerInfo::new("rust", "ra", "rust-analyzer", vec![]));
        reg.register(LspServerInfo::new("python", "pyright", "pyright", vec![]));
        let langs = reg.languages();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
    }

    #[test]
    fn registry_replaces_duplicate_language() {
        let mut reg = LspServerRegistry::new();
        reg.register(LspServerInfo::new("rust", "old-server", "old", vec![]));
        reg.register(LspServerInfo::new("rust", "new-server", "new", vec![]));
        assert_eq!(reg.server_count(), 1);
        assert_eq!(reg.get("rust").unwrap().server_name, "new-server");
    }

    // -----------------------------------------------------------------------
    // LspCapabilities / TextDocSyncKind tests
    // -----------------------------------------------------------------------

    #[test]
    fn text_doc_sync_kind_display() {
        assert_eq!(format!("{}", TextDocSyncKind::None), "none");
        assert_eq!(format!("{}", TextDocSyncKind::Full), "full");
        assert_eq!(format!("{}", TextDocSyncKind::Incremental), "incremental");
    }

    #[test]
    fn lsp_capabilities_default_summary() {
        let caps = LspCapabilities::new();
        let s = caps.summary();
        assert!(s.contains("unknown server"));
        assert!(s.contains("v?"));
        assert!(s.contains("sync=none"));
        assert!(s.contains("features: none"));
    }

    #[test]
    fn lsp_capabilities_from_initialize_result() {
        let result = lsp_types::InitializeResult {
            capabilities: lsp_types::ServerCapabilities {
                text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                    lsp_types::TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(lsp_types::CompletionOptions::default()),
                hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(lsp_types::ServerInfo {
                name: "test-server".into(),
                version: Some("1.2.3".into()),
            }),
        };

        let caps = LspCapabilities::from_initialize_result(&result);
        assert_eq!(caps.text_document_sync, TextDocSyncKind::Incremental);
        assert!(caps.flags.completion);
        assert!(caps.flags.hover);
        assert!(!caps.flags.rename);
        assert_eq!(caps.server_name.as_deref(), Some("test-server"));
        assert_eq!(caps.server_version.as_deref(), Some("1.2.3"));

        let summary = caps.summary();
        assert!(summary.contains("test-server"));
        assert!(summary.contains("1.2.3"));
        assert!(summary.contains("incremental"));
    }

    #[test]
    fn lsp_capabilities_display_delegates_to_summary() {
        let caps = LspCapabilities::new();
        assert_eq!(format!("{caps}"), caps.summary());
    }

    // -----------------------------------------------------------------------
    // lsp_initialize_params tests
    // -----------------------------------------------------------------------

    #[test]
    #[allow(deprecated)]
    fn initialize_params_valid() {
        let params =
            lsp_initialize_params("/home/user/project", "vsedit", "0.1.0").unwrap();
        assert_eq!(
            params.root_uri.as_ref().unwrap().as_str(),
            "file:///home/user/project"
        );
        assert_eq!(params.client_info.as_ref().unwrap().name, "vsedit");
        assert_eq!(
            params.client_info.as_ref().unwrap().version.as_deref(),
            Some("0.1.0")
        );
        assert!(params.process_id.is_some());
    }

    #[test]
    fn initialize_params_relative_path_fails() {
        assert!(lsp_initialize_params("relative/path", "c", "0").is_err());
    }

    // -----------------------------------------------------------------------
    // DiagnosticIndex tests
    // -----------------------------------------------------------------------

    fn make_diag(uri: &str, sev: Severity, msg: &str) -> DiagnosticEntry {
        DiagnosticEntry {
            uri: uri.to_string(),
            range: DocRange::new(DocPosition::new(0, 0), DocPosition::new(0, 1)),
            severity: sev,
            message: msg.to_string(),
            source: None,
            code: None,
        }
    }

    #[test]
    fn diagnostic_index_set_get_clear() {
        let mut idx = DiagnosticIndex::new();
        assert_eq!(idx.file_count(), 0);
        assert_eq!(idx.total_count(), 0);

        let d1 = make_diag("file:///a.rs", Severity::Error, "err");
        let d2 = make_diag("file:///a.rs", Severity::Warning, "warn");
        let d3 = make_diag("file:///b.rs", Severity::Hint, "hint");

        idx.set_file("file:///a.rs", vec![d1.clone(), d2.clone()]);
        idx.set_file("file:///b.rs", vec![d3.clone()]);

        assert_eq!(idx.file_count(), 2);
        assert_eq!(idx.total_count(), 3);
        assert_eq!(idx.get_for_file("file:///a.rs").len(), 2);
        assert_eq!(idx.get_for_file("file:///c.rs").len(), 0);

        let errors = idx.files_with_errors();
        assert_eq!(errors.len(), 1);
        assert!(errors.contains(&"file:///a.rs"));

        idx.clear_file("file:///a.rs");
        assert_eq!(idx.file_count(), 1);
        assert_eq!(idx.total_count(), 1);

        idx.clear_all();
        assert!(idx.file_count() == 0);
    }

    #[test]
    fn diagnostic_index_from_collection() {
        let mut coll = DiagnosticCollection::new();
        let d1 = make_diag("file:///x.rs", Severity::Error, "e1");
        let d2 = make_diag("file:///y.rs", Severity::Warning, "w1");
        coll.set_for_uri("file:///x.rs", vec![d1]);
        coll.set_for_uri("file:///y.rs", vec![d2]);

        let idx = DiagnosticIndex::from(coll);
        assert_eq!(idx.file_count(), 2);
        assert_eq!(idx.total_count(), 2);
        assert_eq!(idx.count_by_severity(Severity::Error), 1);
    }

    #[test]
    fn diagnostic_index_display() {
        let mut idx = DiagnosticIndex::new();
        idx.set_file(
            "file:///a.rs",
            vec![make_diag("file:///a.rs", Severity::Error, "e")],
        );
        let display = format!("{idx}");
        assert!(display.contains("1 diagnostic(s)"));
        assert!(display.contains("1 file(s)"));
    }

    #[test]
    fn diagnostic_index_set_empty_removes() {
        let mut idx = DiagnosticIndex::new();
        idx.set_file(
            "file:///a.rs",
            vec![make_diag("file:///a.rs", Severity::Info, "i")],
        );
        assert_eq!(idx.file_count(), 1);
        idx.set_file("file:///a.rs", vec![]);
        assert_eq!(idx.file_count(), 0);
    }

    // -----------------------------------------------------------------------
    // CompletionItemEntry tests
    // -----------------------------------------------------------------------

    #[test]
    fn completion_item_entry_matching() {
        let item = CompletionItemEntry {
            label: "HashMap".to_string(),
            kind: CompletionKind::Struct,
            detail: Some("std::collections".to_string()),
            sort_text: None,
            insert_text: None,
        };

        assert!(item.matches_prefix("Hash"));
        assert!(item.matches_prefix("hash"));
        assert!(!item.matches_prefix("Vec"));

        assert!(item.matches_fuzzy("HM"));
        assert!(item.matches_fuzzy("hm"));
        assert!(item.matches_fuzzy("hashmap"));
        assert!(!item.matches_fuzzy("xyz"));

        assert_eq!(item.text_to_insert(), "HashMap");
        assert_eq!(item.sort_key(), "HashMap");
    }

    #[test]
    fn completion_item_entry_with_insert_text() {
        let item = CompletionItemEntry {
            label: "println!".to_string(),
            kind: CompletionKind::Snippet,
            detail: None,
            sort_text: Some("0001".to_string()),
            insert_text: Some("println!(\"{}\", )".to_string()),
        };

        assert_eq!(item.text_to_insert(), "println!(\"{}\", )");
        assert_eq!(item.sort_key(), "0001");
        // prefix match also checks insert_text
        assert!(item.matches_prefix("println"));
    }

    #[test]
    fn completion_item_from_lsp() {
        let lsp_item = lsp_types::CompletionItem {
            label: "my_func".to_string(),
            kind: Some(lsp_types::CompletionItemKind::FUNCTION),
            detail: Some("fn()".to_string()),
            sort_text: None,
            insert_text: Some("my_func()".to_string()),
            ..Default::default()
        };
        let entry = CompletionItemEntry::from(&lsp_item);
        assert_eq!(entry.label, "my_func");
        assert_eq!(entry.kind, CompletionKind::Function);
        assert_eq!(entry.detail.as_deref(), Some("fn()"));
        assert_eq!(entry.insert_text.as_deref(), Some("my_func()"));
    }

    #[test]
    fn completion_item_display() {
        let item = CompletionItemEntry::new("foo", CompletionKind::Function);
        assert_eq!(format!("{item}"), "[f] foo");

        let mut item2 = CompletionItemEntry::new("bar", CompletionKind::Variable);
        item2.detail = Some("i32".to_string());
        assert_eq!(format!("{item2}"), "[v] bar — i32");
    }

    // -----------------------------------------------------------------------
    // CompletionList tests
    // -----------------------------------------------------------------------

    #[test]
    fn completion_list_filter_and_sort() {
        let items = vec![
            CompletionItemEntry::new("zebra", CompletionKind::Variable),
            CompletionItemEntry::new("apple", CompletionKind::Function),
            CompletionItemEntry::new("apricot", CompletionKind::Function),
            CompletionItemEntry::new("banana", CompletionKind::Keyword),
        ];
        let list = CompletionList::new(items, true);

        assert_eq!(list.len(), 4);
        assert!(list.is_incomplete);

        let filtered = list.filter_prefix("ap");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.items.iter().all(|i| i.label.starts_with("ap")));

        let sorted = list.sorted();
        assert_eq!(sorted.items[0].label, "apple");
        assert_eq!(sorted.items[3].label, "zebra");

        let funcs = list.filter_kind(CompletionKind::Function);
        assert_eq!(funcs.len(), 2);

        let fuzzy = list.filter_fuzzy("zb");
        assert_eq!(fuzzy.len(), 1);
        assert_eq!(fuzzy.items[0].label, "zebra");
    }

    #[test]
    fn completion_list_display() {
        let list = CompletionList::new(vec![], false);
        assert_eq!(format!("{list}"), "0 item(s)");

        let list2 = CompletionList::new(
            vec![CompletionItemEntry::new("x", CompletionKind::Text)],
            true,
        );
        assert_eq!(format!("{list2}"), "1 item(s) (incomplete)");
    }

    #[test]
    fn completion_list_from_lsp_array() {
        let lsp_items = vec![
            lsp_types::CompletionItem {
                label: "a".to_string(),
                kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "b".to_string(),
                ..Default::default()
            },
        ];
        let resp = lsp_types::CompletionResponse::Array(lsp_items);
        let list = CompletionList::from(resp);
        assert_eq!(list.len(), 2);
        assert!(!list.is_incomplete);
        assert_eq!(list.items[0].kind, CompletionKind::Keyword);
        assert_eq!(list.items[1].kind, CompletionKind::Text);
    }

    #[test]
    fn completion_kind_display_and_symbol() {
        assert_eq!(CompletionKind::Function.symbol(), 'f');
        assert_eq!(CompletionKind::Variable.symbol(), 'v');
        assert_eq!(format!("{}", CompletionKind::Struct), "struct");
        assert_eq!(format!("{}", CompletionKind::EnumMember), "enum member");
    }

    // -----------------------------------------------------------------------
    // DiagnosticSeverityLevel & LspDiagnosticFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn severity_level_ordering() {
        assert!(DiagnosticSeverityLevel::Error < DiagnosticSeverityLevel::Warning);
        assert!(DiagnosticSeverityLevel::Warning < DiagnosticSeverityLevel::Information);
        assert!(DiagnosticSeverityLevel::Information < DiagnosticSeverityLevel::Hint);
    }

    #[test]
    fn severity_level_display() {
        assert_eq!(format!("{}", DiagnosticSeverityLevel::Error), "error");
        assert_eq!(format!("{}", DiagnosticSeverityLevel::Warning), "warning");
        assert_eq!(
            format!("{}", DiagnosticSeverityLevel::Information),
            "information"
        );
        assert_eq!(format!("{}", DiagnosticSeverityLevel::Hint), "hint");
    }

    #[test]
    fn diagnostic_filter_severity() {
        let filter = LspDiagnosticFilter::new(DiagnosticSeverityLevel::Warning);
        assert!(filter.should_show(&DiagnosticSeverityLevel::Error, None, None));
        assert!(filter.should_show(&DiagnosticSeverityLevel::Warning, None, None));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Information, None, None));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Hint, None, None));
    }

    #[test]
    fn diagnostic_filter_ignored_codes() {
        let mut filter = LspDiagnosticFilter::new(DiagnosticSeverityLevel::Hint);
        filter.ignore_code("E0001");
        filter.ignore_code("W0042");

        assert!(filter.should_show(&DiagnosticSeverityLevel::Error, Some("E9999"), None));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Error, Some("E0001"), None));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Warning, Some("W0042"), None));
        // No code provided — not ignored.
        assert!(filter.should_show(&DiagnosticSeverityLevel::Warning, None, None));
    }

    #[test]
    fn diagnostic_filter_source() {
        let mut filter = LspDiagnosticFilter::new(DiagnosticSeverityLevel::Hint);
        filter.set_source_filter("rustc");

        assert!(filter.should_show(&DiagnosticSeverityLevel::Error, None, Some("rustc")));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Error, None, Some("clippy")));
        assert!(!filter.should_show(&DiagnosticSeverityLevel::Error, None, None));
    }

    // -----------------------------------------------------------------------
    // LspCodeActionQuickFix tests
    // -----------------------------------------------------------------------

    #[test]
    fn code_action_quick_fix_builder() {
        let action = LspCodeActionQuickFix::new("Add import", "quickfix.import")
            .with_preferred()
            .with_edits(3);

        assert!(action.is_quick_fix());
        assert!(action.is_preferred);
        assert_eq!(action.edit_count, 3);
        assert!(action.summary().contains("[preferred]"));
        assert!(action.summary().contains("3 edit(s)"));
    }

    #[test]
    fn code_action_non_quickfix() {
        let action = LspCodeActionQuickFix::new("Refactor", "refactor.extract");
        assert!(!action.is_quick_fix());
        assert!(!action.is_preferred);
        let display = format!("{action}");
        assert!(display.contains("Refactor"));
        assert!(!display.contains("[preferred]"));
    }

    // -----------------------------------------------------------------------
    // WorkspaceSymbol & LspWorkspaceSymbolSearch tests
    // -----------------------------------------------------------------------

    #[test]
    fn workspace_symbol_display() {
        let sym = WorkspaceSymbol {
            name: "MyStruct".into(),
            kind: "struct".into(),
            location: "src/lib.rs:10".into(),
            container: Some("my_mod".into()),
        };
        let s = format!("{sym}");
        assert!(s.contains("MyStruct"));
        assert!(s.contains("[struct]"));
        assert!(s.contains("(in my_mod)"));
    }

    #[test]
    fn workspace_symbol_search_filter() {
        let mut search = LspWorkspaceSymbolSearch::new("Foo");
        assert!(!search.has_results());
        assert_eq!(search.result_count(), 0);

        search.add_result(WorkspaceSymbol {
            name: "FooBar".into(),
            kind: "function".into(),
            location: "a.rs:1".into(),
            container: None,
        });
        search.add_result(WorkspaceSymbol {
            name: "FooBaz".into(),
            kind: "struct".into(),
            location: "b.rs:2".into(),
            container: None,
        });
        search.add_result(WorkspaceSymbol {
            name: "FooQux".into(),
            kind: "function".into(),
            location: "c.rs:3".into(),
            container: None,
        });

        assert!(search.has_results());
        assert_eq!(search.result_count(), 3);
        let funcs = search.filter_by_kind("function");
        assert_eq!(funcs.len(), 2);
        let structs = search.filter_by_kind("struct");
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "FooBaz");
    }

    // -----------------------------------------------------------------------
    // LspProgressTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn progress_tracker_lifecycle() {
        let mut tracker = LspProgressTracker::new();
        assert_eq!(tracker.active_count(), 0);
        assert!(tracker.is_done("tok1"));

        tracker.begin("tok1", "Indexing");
        assert_eq!(tracker.active_count(), 1);
        assert!(!tracker.is_done("tok1"));

        tracker.report("tok1", Some("50%"), Some(50));
        assert_eq!(tracker.tokens["tok1"].percentage, Some(50));

        tracker.end("tok1");
        assert!(tracker.is_done("tok1"));
        assert_eq!(tracker.active_count(), 0);
        assert_eq!(tracker.tokens["tok1"].percentage, Some(100));
    }

    #[test]
    fn progress_tracker_render_bar() {
        let bar = LspProgressTracker::render_progress_bar(0, 10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));

        let full = LspProgressTracker::render_progress_bar(100, 10);
        assert!(full.contains("=========="));

        let half = LspProgressTracker::render_progress_bar(50, 10);
        assert!(half.contains('>'));
    }

    #[test]
    fn progress_tracker_default() {
        let tracker = LspProgressTracker::default();
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn workspace_folders_sync_add() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///project", "project", 100);
        assert_eq!(sync.folder_count(), 1);
        assert!(sync.has_pending());
    }

    #[test]
    fn workspace_folders_sync_no_dup() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///a", "a", 1);
        sync.add_folder("file:///a", "a", 2);
        assert_eq!(sync.folder_count(), 1);
    }

    #[test]
    fn workspace_folders_sync_remove() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///a", "a", 1);
        assert!(sync.remove_folder("file:///a"));
        assert_eq!(sync.folder_count(), 0);
        assert!(!sync.remove_folder("file:///b"));
    }

    #[test]
    fn workspace_folders_sync_drain() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///a", "a", 1);
        sync.remove_folder("file:///a");
        let changes = sync.drain_pending();
        assert_eq!(changes.len(), 2);
        assert!(!sync.has_pending());
        assert_eq!(sync.sync_count(), 1);
    }

    #[test]
    fn workspace_folders_sync_find() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///x", "x", 10);
        let f = sync.find("file:///x").unwrap();
        assert_eq!(f.name, "x");
        assert_eq!(f.added_at, 10);
        assert!(sync.find("file:///y").is_none());
    }

    #[test]
    fn workspace_folders_sync_change_event() {
        let mut sync = LspWorkspaceFoldersSync::new();
        sync.add_folder("file:///a", "a", 1);
        sync.add_folder("file:///b", "b", 2);
        sync.remove_folder("file:///a");
        let (added, removed) = sync.build_change_event();
        assert_eq!(added.len(), 2);
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn file_operation_handler_create() {
        let mut h = LspFileOperationHandler::new();
        h.record_create("file:///new.rs");
        assert_eq!(h.total_count(), 1);
        assert_eq!(h.create_count(), 1);
    }

    #[test]
    fn file_operation_handler_rename() {
        let mut h = LspFileOperationHandler::new();
        h.record_rename("file:///old.rs", "file:///new.rs");
        assert_eq!(h.rename_count(), 1);
        let renames = h.filter_by_kind(FileOperationKind::Rename);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].new_uri.as_deref(), Some("file:///new.rs"));
    }

    #[test]
    fn file_operation_handler_delete() {
        let mut h = LspFileOperationHandler::new();
        h.record_delete("file:///gone.rs");
        assert_eq!(h.delete_count(), 1);
    }

    #[test]
    fn file_operation_handler_is_affected() {
        let mut h = LspFileOperationHandler::new();
        h.record_rename("file:///a.rs", "file:///b.rs");
        assert!(h.is_affected("file:///a.rs"));
        assert!(h.is_affected("file:///b.rs"));
        assert!(!h.is_affected("file:///c.rs"));
    }

    #[test]
    fn file_operation_handler_drain() {
        let mut h = LspFileOperationHandler::new();
        h.record_create("file:///x.rs");
        h.record_delete("file:///y.rs");
        let ops = h.drain();
        assert_eq!(ops.len(), 2);
        assert!(h.is_empty());
    }

    #[test]
    fn file_operation_handler_empty() {
        let h = LspFileOperationHandler::new();
        assert!(h.is_empty());
        assert_eq!(h.total_count(), 0);
    }

}
