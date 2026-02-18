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



// ─── Lsp Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for LSP messages.
#[derive(Debug, Clone)]
pub struct LspRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> LspRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for LspRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LspRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Lsp LRU Cache ───────────────────────────────────────

/// A simple LRU cache for LSP completions.
#[derive(Debug)]
pub struct LspLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> LspLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for LspLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LspLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}



// ---------------------------------------------------------------------------
// lsp – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for language server protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YLspLspServerState {
    Stopped,
    Starting,
    Running,
    Crashed,
}

impl YLspLspServerState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Stopped => 0,
            Self::Starting => 1,
            Self::Running => 2,
            Self::Crashed => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Crashed => "Crashed",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YLspLspServerState] {
        &[
            YLspLspServerState::Stopped,
            YLspLspServerState::Starting,
            YLspLspServerState::Running,
            YLspLspServerState::Crashed,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YLspLspServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks LSP capabilities data.
#[derive(Debug, Clone)]
pub struct YLspLspCapabilitySet {
    pub capabilities: Vec<String>,
    pub version: String,
    pub dynamic: bool,
}

impl YLspLspCapabilitySet {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            version: String::new(),
            dynamic: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.capabilities.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YLspLspCapabilitySet({}: {:?})", "capabilities", self.capabilities)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_lsp_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_lsp_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_lsp_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_lsp_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_lsp_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_lsp_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_lsp_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_lsp_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// lsp – Extended LSP diagnostic batch helpers
// ---------------------------------------------------------------------------

/// Priority levels for LSP diagnostic batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZLspPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZLspPriority {
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
    pub fn all_asc() -> [ZLspPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZLspPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks LSP diagnostic batch data.
#[derive(Debug, Clone)]
pub struct ZLspLspDiagnosticBatch {
    pub items: Vec<(String, u32, String)>,
    pub uri: String,
    pub version: u32,
}

impl ZLspLspDiagnosticBatch {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            uri: String::new(),
            version: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZLspLspDiagnosticBatch[uri={:?}, version={:?}]", self.uri, self.version)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for LSP diagnostic batch.
pub fn z_lsp_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_lsp_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_lsp_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_lsp_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_lsp_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_lsp_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_lsp_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 118
// ---------------------------------------------------------------------------

/// Generic object pool `Xc118Pool<T>`.
pub struct Xc118Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc118Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc118PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc118Pool<T> {
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
    pub fn stats(&self) -> Xc118PoolStats {
        Xc118PoolStats {
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

impl<T> Default for Xc118Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc118Scheduler`.
pub struct Xc118Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc118Scheduler {
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

impl Default for Xc118Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_118 hash for the given byte slice.
pub fn xc_118_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_118 convention.
pub fn xc_118_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_1 deepening: state machine + event bus ---

/// States for the Xd1 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd1State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd1State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd1Transition {
    pub from: Xd1State,
    pub to: Xd1State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd1StateMachine {
    current: Xd1State,
    history: Vec<Xd1Transition>,
    step_counter: usize,
}

impl Xd1StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd1State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd1State {
        self.current
    }

    pub fn history(&self) -> &[Xd1Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd1State) -> Result<Xd1State, String> {
        let allowed = match (self.current, target) {
            (Xd1State::Idle, Xd1State::Running) => true,
            (Xd1State::Running, Xd1State::Paused) => true,
            (Xd1State::Running, Xd1State::Done) => true,
            (Xd1State::Paused, Xd1State::Running) => true,
            (Xd1State::Paused, Xd1State::Done) => true,
            (Xd1State::Done, Xd1State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_1: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd1Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd1SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd1State> {
        let prefix = "Xd1SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd1State::Idle),
            "Running" => Some(Xd1State::Running),
            "Paused" => Some(Xd1State::Paused),
            "Done" => Some(Xd1State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd1State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd1 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd1Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd1Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd1HandlerFn = Box<dyn Fn(&Xd1Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd1EventBus {
    handlers: Vec<(usize, Option<String>, Xd1HandlerFn)>,
    next_id: usize,
    published: Vec<Xd1Event>,
}

impl Xd1EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd1Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd1Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd1Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd1Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// === Xe120 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe120Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe120PipelineError {
    pub stage: Xe120Stage,
    pub message: String,
}

impl std::fmt::Display for Xe120PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe120Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe120Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError>>>,
    stage_names: Vec<Xe120Stage>,
}

impl Xe120Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe120Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe120Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe120Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe120Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
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

    pub fn compose(mut self, other: Xe120Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe120CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe120CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe120Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe120CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe120CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe120Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe120CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_120_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe120CacheEntry {
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

    fn xe_120_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe120CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_120_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
    Ok(data)
}

pub fn xe_120_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_120_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_120_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_120_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe120PipelineError> {
    Err(Xe120PipelineError {
        stage: Xe120Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_118: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg118Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg118Graph {
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

impl Default for Xg118Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_118: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg118Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg118Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg118Heap<T>) {
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

impl<T: Ord> Default for Xg118Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 117).
pub struct Xh117SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh117SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 159 as u64,
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

/// A compact bit set supporting boolean operations (variant 117).
pub struct Xh117BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh117BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 117).
pub struct Xi117Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi117Deque<T> {
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
pub struct Xi117Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi117Interval {
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

/// A simple interval tree (variant 117).
pub struct Xi117IntervalTree {
    xi_intervals: Vec<Xi117Interval>,
}

impl Xi117IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi117Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi117Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi117Interval) -> Vec<&Xi117Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi117Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi117Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi117Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi117Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi117Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi117Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 117) ---

/// Disjoint set / union-find for crate 117.
pub struct Xj117UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj117UnionFind {
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

const XJ117_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 117.
pub struct Xj117BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj117BTreeNode<K, V>>>,
    len: usize,
}

struct Xj117BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj117BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj117BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ117_BTREE_ORDER - 1
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
        let mid = XJ117_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj117BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj117BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj117BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj117BTreeNode::xj_new_leaf();
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


// --- xk_117 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk117SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk117SegmentTree {
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
pub struct Xk117DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk117DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_117).
#[derive(Debug, Clone)]
pub struct Xl117Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl117Rope {
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

/// Suffix array for efficient string searching (xl_117).
#[derive(Debug, Clone)]
pub struct Xl117SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl117SuffixArray {
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
pub struct Xm117MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm117MatrixSparse {
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
pub struct Xm117Tokenizer {
    text: String,
}

impl Xm117Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 117.
pub struct Xn117Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn117Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 117 -----

#[derive(Debug, Clone)]
struct Xn117AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn117AvlNode<K, V>>>,
    right: Option<Box<Xn117AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 117.
#[derive(Debug, Clone)]
pub struct Xn117AVL<K, V> {
    root: Option<Box<Xn117AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn117AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn117AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn117AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn117AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn117AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn117AvlNode<K, V>>) -> Box<Xn117AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn117AvlNode<K, V>>) -> Box<Xn117AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn117AvlNode<K, V>>) -> Box<Xn117AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn117AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn117AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn117AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn117AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn117AvlNode<K, V>>) -> &Xn117AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn117AvlNode<K, V>>) -> (Box<Xn117AvlNode<K, V>>, Option<Box<Xn117AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn117AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn117AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn117AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn117AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn117AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn117AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn117AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo117RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo117Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo117RBNode<K, V> {
    key: K,
    value: V,
    color: Xo117Color,
    left: Option<Box<Xo117RBNode<K, V>>>,
    right: Option<Box<Xo117RBNode<K, V>>>,
}

/// A red-black tree map for crate 117.
#[derive(Debug, Clone)]
pub struct Xo117RedBlack<K, V> {
    root: Option<Box<Xo117RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo117RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo117Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo117RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo117RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo117RBNode {
                    key, value, color: Xo117Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo117RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo117Color::Red)
    }

    fn xo_balance(mut h: Box<Xo117RBNode<K, V>>) -> Box<Xo117RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo117Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo117RBNode<K, V>>) -> Box<Xo117RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo117Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo117RBNode<K, V>>) -> Box<Xo117RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo117Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo117RBNode<K, V>>) {
        h.color = Xo117Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo117Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo117Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo117Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo117RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo117RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo117RBNode<K, V>) -> (K, V, Option<Box<Xo117RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo117RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo117Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo117RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo117ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 117.
#[derive(Debug, Clone)]
pub struct Xo117ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo117ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo117#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo117#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 117).
#[derive(Debug)]
pub struct Xp117SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp117Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp117Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp117Node<K, V>>>,
    xp_right: Option<Box<Xp117Node<K, V>>>,
}

impl<K: Ord, V> Xp117Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp117SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp117SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp117Node<K, V>>>, key: &K) -> Option<Box<Xp117Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp117Node<K, V>>) -> Box<Xp117Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp117Node<K, V>>) -> Box<Xp117Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp117Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp117Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp117Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq117Treap ---------------

use std::cmp::Ordering as Xq117Ord;

struct Xq117TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq117TreapNode<K, V>>>,
    right: Option<Box<Xq117TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq117Treap<K, V> {
    root: Option<Box<Xq117TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq117TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_117_size<K, V>(node: &Option<Box<Xq117TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_117_update_size<K, V>(node: &mut Xq117TreapNode<K, V>) {
    node.size = 1 + xq_117_size(&node.left) + xq_117_size(&node.right);
}

fn xq_117_rotate_right<K, V>(mut node: Box<Xq117TreapNode<K, V>>) -> Box<Xq117TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_117_update_size(&mut node);
    left.right = Some(node);
    xq_117_update_size(&mut left);
    left
}

fn xq_117_rotate_left<K, V>(mut node: Box<Xq117TreapNode<K, V>>) -> Box<Xq117TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_117_update_size(&mut node);
    right.left = Some(node);
    xq_117_update_size(&mut right);
    right
}

fn xq_117_insert_node<K: Ord, V>(
    node: Option<Box<Xq117TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq117TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq117TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq117Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq117Ord::Less => {
                let (new_left, old) = xq_117_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_117_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_117_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq117Ord::Greater => {
                let (new_right, old) = xq_117_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_117_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_117_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_117_remove_node<K: Ord, V>(
    node: Option<Box<Xq117TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq117TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq117Ord::Less => {
                let (new_left, old) = xq_117_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_117_update_size(&mut n);
                (Some(n), old)
            }
            Xq117Ord::Greater => {
                let (new_right, old) = xq_117_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_117_update_size(&mut n);
                (Some(n), old)
            }
            Xq117Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_117_rotate_right(n);
                    let (new_right, old) = xq_117_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_117_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_117_rotate_left(n);
                    let (new_left, old) = xq_117_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_117_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_117_find_min<K, V>(node: &Option<Box<Xq117TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_117_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_117_find_max<K, V>(node: &Option<Box<Xq117TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_117_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_117_rank<K: Ord, V>(node: &Option<Box<Xq117TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq117Ord::Less => xq_117_rank(&n.left, key),
            Xq117Ord::Equal => xq_117_size(&n.left),
            Xq117Ord::Greater => 1 + xq_117_size(&n.left) + xq_117_rank(&n.right, key),
        },
    }
}

fn xq_117_kth<K, V>(node: &Option<Box<Xq117TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_117_size(&n.left);
        if k < left_size {
            xq_117_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_117_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_117_in_order<K: Clone, V>(node: &Option<Box<Xq117TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_117_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_117_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq117Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 117 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_117_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq117Ord::Equal => return Some(&n.value),
                Xq117Ord::Less => cur = &n.left,
                Xq117Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_117_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_117_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_117_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_117_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_117_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_117_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_117_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq117VEBTree ---------------

pub struct Xq117VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq117VEBTree>>,
    clusters: Vec<Option<Box<Xq117VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq117VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq117VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq117VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
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


    #[test]
    fn lsp_ringbuf_push_get() {
        let mut rb = LspRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn lsp_ringbuf_overflow() {
        let mut rb = LspRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn lsp_ringbuf_clear() {
        let mut rb = LspRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn lsp_ringbuf_newest_oldest() {
        let mut rb = LspRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn lsp_ringbuf_to_vec() {
        let mut rb = LspRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn lsp_ringbuf_is_full() {
        let mut rb = LspRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn lsp_lru_insert_get() {
        let mut c = LspLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn lsp_lru_eviction() {
        let mut c = LspLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn lsp_lru_hit_ratio() {
        let mut c = LspLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn lsp_lru_clear() {
        let mut c = LspLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn lsp_lru_remove() {
        let mut c = LspLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn lsp_lru_peek() {
        let mut c = LspLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    // -- lsp extended domain tests ----------------------------------------

    #[test]
    fn y_lsp_enum_index() {
        assert_eq!(YLspLspServerState::Stopped.index(), 0);
        assert_eq!(YLspLspServerState::Starting.index(), 1);
        assert_eq!(YLspLspServerState::Running.index(), 2);
        assert_eq!(YLspLspServerState::Crashed.index(), 3);
    }

    #[test]
    fn y_lsp_enum_label() {
        assert_eq!(YLspLspServerState::Stopped.label(), "Stopped");
        assert_eq!(YLspLspServerState::Starting.label(), "Starting");
        assert_eq!(YLspLspServerState::Running.label(), "Running");
        assert_eq!(YLspLspServerState::Crashed.label(), "Crashed");
    }

    #[test]
    fn y_lsp_enum_all() {
        let all = YLspLspServerState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_lsp_enum_is_default() {
        assert!(YLspLspServerState::Stopped.is_default());
        assert!(!YLspLspServerState::Crashed.is_default());
    }

    #[test]
    fn y_lsp_enum_display() {
        assert_eq!(format!("{}", YLspLspServerState::Stopped), "Stopped");
    }

    #[test]
    fn y_lsp_struct_new() {
        let s = YLspLspCapabilitySet::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_lsp_struct_clear() {
        let mut s = YLspLspCapabilitySet::new();
        s.capabilities.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_lsp_fingerprint_deterministic() {
        let h1 = y_lsp_fingerprint("hello");
        let h2 = y_lsp_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_lsp_fingerprint("a"), y_lsp_fingerprint("b"));
    }

    #[test]
    fn y_lsp_truncate_short() {
        assert_eq!(y_lsp_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_lsp_truncate_long() {
        let r = y_lsp_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_lsp_normalize_key_basic() {
        assert_eq!(y_lsp_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_lsp_split_path_basic() {
        let parts = y_lsp_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_lsp_count_occurrences_basic() {
        assert_eq!(y_lsp_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_lsp_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_lsp_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_lsp_in_range_basic() {
        assert!(y_lsp_in_range(5, 1, 10));
        assert!(y_lsp_in_range(1, 1, 10));
        assert!(y_lsp_in_range(10, 1, 10));
        assert!(!y_lsp_in_range(0, 1, 10));
        assert!(!y_lsp_in_range(11, 1, 10));
    }

    #[test]
    fn y_lsp_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_lsp_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_lsp_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_lsp_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- lsp Z-extended tests -----------------------------------------------

    #[test]
    fn z_lsp_priority_weight() {
        assert_eq!(ZLspPriority::Idle.weight(), 0);
        assert_eq!(ZLspPriority::Normal.weight(), 2);
        assert_eq!(ZLspPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_lsp_priority_label() {
        assert_eq!(ZLspPriority::Low.label(), "low");
        assert_eq!(ZLspPriority::High.label(), "high");
    }

    #[test]
    fn z_lsp_priority_is_elevated() {
        assert!(!ZLspPriority::Normal.is_elevated());
        assert!(ZLspPriority::High.is_elevated());
        assert!(ZLspPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_lsp_priority_display() {
        assert_eq!(format!("{}", ZLspPriority::Idle), "idle");
    }

    #[test]
    fn z_lsp_priority_all_asc() {
        let all = ZLspPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZLspPriority::Idle);
        assert_eq!(all[4], ZLspPriority::Realtime);
    }

    #[test]
    fn z_lsp_struct_new() {
        let s = ZLspLspDiagnosticBatch::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_lsp_struct_toggled_clone() {
        let s = ZLspLspDiagnosticBatch::new();
        let t = s.toggled_clone();
        let _ = t.version;
    }

    #[test]
    fn z_lsp_rolling_hash_deterministic() {
        let h1 = z_lsp_rolling_hash(b"test");
        let h2 = z_lsp_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_lsp_rolling_hash(b"a"), z_lsp_rolling_hash(b"b"));
    }

    #[test]
    fn z_lsp_pad_to_basic() {
        assert_eq!(z_lsp_pad_to("hi", 5), "hi   ");
        assert_eq!(z_lsp_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_lsp_is_identifier_basic() {
        assert!(z_lsp_is_identifier("foo_bar"));
        assert!(z_lsp_is_identifier("abc123"));
        assert!(!z_lsp_is_identifier(""));
        assert!(!z_lsp_is_identifier("has space"));
    }

    #[test]
    fn z_lsp_levenshtein_basic() {
        assert_eq!(z_lsp_levenshtein("", ""), 0);
        assert_eq!(z_lsp_levenshtein("abc", "abc"), 0);
        assert_eq!(z_lsp_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_lsp_unique_words_basic() {
        let w = z_lsp_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_lsp_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_lsp_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_lsp_common_prefix_basic() {
        assert_eq!(z_lsp_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_lsp_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_lsp_struct_clear() {
        let mut s = ZLspLspDiagnosticBatch::new();
        s.items.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_lsp_rolling_hash_empty() {
        let h = z_lsp_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 118 ----

    #[test]
    fn xc_118_pool_new_empty() {
        let pool: super::Xc118Pool<i32> = super::Xc118Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_118_pool_release_acquire() {
        let mut pool = super::Xc118Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_118_pool_acquire_empty() {
        let mut pool: super::Xc118Pool<i32> = super::Xc118Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_118_pool_full() {
        let mut pool = super::Xc118Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_118_pool_drain() {
        let mut pool = super::Xc118Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_118_pool_stats() {
        let mut pool = super::Xc118Pool::new(8);
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
    fn xc_118_pool_clear() {
        let mut pool = super::Xc118Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_118_pool_shrink() {
        let mut pool = super::Xc118Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_118_pool_default() {
        let pool: super::Xc118Pool<String> = super::Xc118Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_118_pool_extend() {
        let mut pool = super::Xc118Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_118_pool_retain() {
        let mut pool = super::Xc118Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_118_scheduler_round_robin() {
        let mut sched = super::Xc118Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_118_scheduler_empty() {
        let mut sched = super::Xc118Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_118_scheduler_reset() {
        let mut sched = super::Xc118Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_118_scheduler_add_remove() {
        let mut sched = super::Xc118Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_118_scheduler_targets() {
        let sched = super::Xc118Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_118_hash_empty() {
        assert_eq!(super::xc_118_hash(b""), 5381);
    }

    #[test]
    fn xc_118_hash_data() {
        let h = super::xc_118_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_118_hash(b"hello"), h);
    }

    #[test]
    fn xc_118_reverse_str() {
        assert_eq!(super::xc_118_reverse("abc"), "cba");
        assert_eq!(super::xc_118_reverse(""), "");
    }


    // --- xd_1 deepening tests ---

    #[test]
    fn xd_1_sm_initial_state() {
        let sm = Xd1StateMachine::new();
        assert_eq!(sm.current_state(), Xd1State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_1_sm_valid_idle_to_running() {
        let mut sm = Xd1StateMachine::new();
        assert!(sm.transition(Xd1State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd1State::Running);
    }

    #[test]
    fn xd_1_sm_valid_running_to_paused() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        assert!(sm.transition(Xd1State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd1State::Paused);
    }

    #[test]
    fn xd_1_sm_valid_running_to_done() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        assert!(sm.transition(Xd1State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd1State::Done);
    }

    #[test]
    fn xd_1_sm_valid_paused_to_running() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        sm.transition(Xd1State::Paused).unwrap();
        assert!(sm.transition(Xd1State::Running).is_ok());
    }

    #[test]
    fn xd_1_sm_valid_done_to_idle() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        sm.transition(Xd1State::Done).unwrap();
        assert!(sm.transition(Xd1State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd1State::Idle);
    }

    #[test]
    fn xd_1_sm_invalid_idle_to_done() {
        let mut sm = Xd1StateMachine::new();
        assert!(sm.transition(Xd1State::Done).is_err());
    }

    #[test]
    fn xd_1_sm_invalid_idle_to_paused() {
        let mut sm = Xd1StateMachine::new();
        assert!(sm.transition(Xd1State::Paused).is_err());
    }

    #[test]
    fn xd_1_sm_history_tracking() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        sm.transition(Xd1State::Paused).unwrap();
        sm.transition(Xd1State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd1State::Idle);
        assert_eq!(sm.history()[0].to, Xd1State::Running);
        assert_eq!(sm.history()[1].from, Xd1State::Running);
        assert_eq!(sm.history()[2].to, Xd1State::Done);
    }

    #[test]
    fn xd_1_sm_serialize_deserialize() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd1StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd1State::Running));
    }

    #[test]
    fn xd_1_sm_deserialize_invalid() {
        assert_eq!(Xd1StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_1_sm_reset() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd1State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_1_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd1EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd1Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_1_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd1EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd1Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd1Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_1_bus_unsubscribe() {
        let mut bus = Xd1EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_1_event_kind_and_payload() {
        let e = Xd1Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd1Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_1_bus_clear_history() {
        let mut bus = Xd1EventBus::new();
        bus.publish(Xd1Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_1_sm_step_counter_increments() {
        let mut sm = Xd1StateMachine::new();
        sm.transition(Xd1State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd1State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    #[test]
    fn xe_120_pipeline_empty() {
        let p = super::Xe120Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_120_pipeline_parse_stage() {
        let p = super::Xe120Pipeline::new()
            .add_parse(super::xe_120_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_120_pipeline_transform_double() {
        let p = super::Xe120Pipeline::new()
            .add_transform(super::xe_120_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_120_pipeline_validate_reverse() {
        let p = super::Xe120Pipeline::new()
            .add_validate(super::xe_120_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_120_pipeline_emit_filter() {
        let p = super::Xe120Pipeline::new()
            .add_emit(super::xe_120_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_120_pipeline_multi_stage() {
        let p = super::Xe120Pipeline::new()
            .add_parse(super::xe_120_pipeline_identity)
            .add_transform(super::xe_120_pipeline_double)
            .add_validate(super::xe_120_pipeline_reverse)
            .add_emit(super::xe_120_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_120_pipeline_error_propagation() {
        let p = super::Xe120Pipeline::new()
            .add_parse(super::xe_120_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe120Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_120_pipeline_compose() {
        let p1 = super::Xe120Pipeline::new()
            .add_parse(super::xe_120_pipeline_identity);
        let p2 = super::Xe120Pipeline::new()
            .add_transform(super::xe_120_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_120_pipeline_error_display() {
        let e = super::Xe120PipelineError {
            stage: super::Xe120Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_120_cache_put_get() {
        let mut c = super::Xe120Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_120_cache_miss() {
        let mut c: super::Xe120Cache<&str, i32> = super::Xe120Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_120_cache_ttl_expiry() {
        let mut c = super::Xe120Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_120_cache_evict() {
        let mut c = super::Xe120Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_120_cache_capacity() {
        let mut c = super::Xe120Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_120_cache_stats() {
        let mut c = super::Xe120Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_120_cache_clear() {
        let mut c = super::Xe120Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_118 graph tests ------------------------------------------------

    #[test]
    fn xg_118_graph_empty() {
        let g = super::Xg118Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_118_graph_add_node() {
        let mut g = super::Xg118Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_118_graph_add_edge() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_118_graph_neighbors() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_118_graph_has_path() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_118_graph_self_path() {
        let g = super::Xg118Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_118_graph_topo_sort() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_118_graph_cycle_detect_false() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_118_graph_cycle_detect_true() {
        let mut g = super::Xg118Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_118 heap tests -------------------------------------------------

    #[test]
    fn xg_118_heap_empty() {
        let h: super::Xg118Heap<i32> = super::Xg118Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_118_heap_push_pop() {
        let mut h = super::Xg118Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_118_heap_peek() {
        let mut h = super::Xg118Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_118_heap_drain_sorted() {
        let mut h = super::Xg118Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_118_heap_merge() {
        let mut a = super::Xg118Heap::new();
        let mut b = super::Xg118Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_118_heap_default() {
        let h: super::Xg118Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_118_graph_default() {
        let g: super::Xg118Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh117_skip_insert_contains() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh117_skip_remove() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh117_skip_len() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh117_skip_range_query() {
        let mut sl = super::Xh117SkipList::xh_new(4);
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
    fn xh117_skip_floor_ceiling() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh117_skip_rank() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh117_skip_empty() {
        let sl = super::Xh117SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh117_skip_duplicates() {
        let mut sl = super::Xh117SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh117_bitset_set_test() {
        let mut bs = super::Xh117BitSet::xh_new(256);
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
    fn xh117_bitset_clear_count() {
        let mut bs = super::Xh117BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh117_bitset_and_or_xor() {
        let mut a = super::Xh117BitSet::xh_new(128);
        let mut b = super::Xh117BitSet::xh_new(128);
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
    fn xh117_bitset_iter_ones() {
        let mut bs = super::Xh117BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh117_bitset_first_last() {
        let mut bs = super::Xh117BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh117_bitset_empty() {
        let bs = super::Xh117BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi117_deque_push_pop_back() {
        let mut dq = super::Xi117Deque::xi_new(4);
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
    fn xi117_deque_push_pop_front() {
        let mut dq = super::Xi117Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi117_deque_mixed_ops() {
        let mut dq = super::Xi117Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi117_deque_get_and_split() {
        let mut dq = super::Xi117Deque::xi_new(8);
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
    fn xi117_deque_rotate_left() {
        let mut dq = super::Xi117Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi117_deque_rotate_right() {
        let mut dq = super::Xi117Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi117_deque_grow() {
        let mut dq = super::Xi117Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi117_deque_empty() {
        let dq = super::Xi117Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi117_interval_tree_insert_query() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi117Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi117Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi117_interval_tree_overlap() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi117Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi117Interval::xi_new(12, 20));
        let q = super::Xi117Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi117_interval_tree_remove() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi117Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi117_interval_tree_gaps() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi117Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi117Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi117Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi117Interval::xi_new(8, 10));
    }

    #[test]
    fn xi117_interval_tree_merge() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi117Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi117Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi117Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi117Interval::xi_new(10, 15));
    }

    #[test]
    fn xi117_interval_tree_all() {
        let mut tree = super::Xi117IntervalTree::xi_new();
        tree.xi_insert(super::Xi117Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi117Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi117_interval_tree_empty() {
        let tree = super::Xi117IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi117_interval_tree_contains_point() {
        let iv = super::Xi117Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 117) ---

    #[test]
    fn xj_117_uf_make_and_find() {
        let mut uf = super::Xj117UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_117_uf_union_connected() {
        let mut uf = super::Xj117UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_117_uf_component_count() {
        let mut uf = super::Xj117UnionFind::xj_new();
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
    fn xj_117_uf_component_size() {
        let mut uf = super::Xj117UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_117_uf_largest_component() {
        let mut uf = super::Xj117UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_117_uf_many_elements() {
        let mut uf = super::Xj117UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_117_uf_separate_components() {
        let mut uf = super::Xj117UnionFind::xj_new();
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
    fn xj_117_uf_path_compression() {
        let mut uf = super::Xj117UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_117_bt_insert_get() {
        let mut bt = super::Xj117BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_117_bt_contains_len() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_117_bt_replace() {
        let mut bt = super::Xj117BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_117_bt_remove() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_117_bt_keys_values() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_117_bt_range() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_117_bt_min_max() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_117_bt_many_inserts() {
        let mut bt = super::Xj117BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_117 segment tree tests ---

    #[test]
    fn xk_117_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_117_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk117SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_117_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_117_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_117_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_117_st_single_element() {
        let data = vec![42];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_117_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk117SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_117_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk117SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_117 disjoint intervals tests ---

    #[test]
    fn xk_117_di_add_and_count() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_117_di_merge_overlap() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_117_di_contains() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_117_di_remove() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_117_di_covered_length() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_117_di_gaps() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_117_di_merge_adjacent() {
        let mut di = super::Xk117DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_117_di_empty() {
        let di = super::Xk117DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_117_rope_new_empty() {
        let rope = super::Xl117Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_117_rope_from_str() {
        let rope = super::Xl117Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_117_rope_insert_at() {
        let mut rope = super::Xl117Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_117_rope_delete_range() {
        let mut rope = super::Xl117Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_117_rope_char_at() {
        let rope = super::Xl117Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_117_rope_split_concat() {
        let rope = super::Xl117Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_117_rope_line_count() {
        let rope = super::Xl117Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_117_rope_line_at() {
        let rope = super::Xl117Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_117_sa_build_and_search() {
        let sa = super::Xl117SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_117_sa_count() {
        let sa = super::Xl117SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_117_sa_longest_repeated() {
        let sa = super::Xl117SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_117_sa_all_positions() {
        let sa = super::Xl117SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_117_sa_len() {
        let sa = super::Xl117SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_117_sa_empty() {
        let sa = super::Xl117SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_117_rope_slice() {
        let rope = super::Xl117Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_117_sa_search_start() {
        let sa = super::Xl117SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_117_sparse_set_get() {
        let mut m = super::Xm117MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_117_sparse_row_col() {
        let mut m = super::Xm117MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_117_sparse_transpose() {
        let mut m = super::Xm117MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_117_sparse_multiply_vec() {
        let mut m = super::Xm117MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_117_sparse_nnz_density() {
        let mut m = super::Xm117MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_117_sparse_clear() {
        let mut m = super::Xm117MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_117_sparse_overwrite_zero() {
        let mut m = super::Xm117MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_117_tokenizer_basic() {
        let t = super::Xm117Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_117_tokenizer_count() {
        let t = super::Xm117Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_117_tokenizer_unique() {
        let t = super::Xm117Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_117_tokenizer_frequency() {
        let t = super::Xm117Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_117_tokenizer_delimiter() {
        let t = super::Xm117Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_117_tokenizer_whitespace() {
        let t = super::Xm117Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_117_tokenizer_empty() {
        let t = super::Xm117Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 117 ----

    #[test]
    fn xn_117_fenwick_prefix_sum() {
        let mut ft = super::Xn117Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_117_fenwick_range_sum() {
        let mut ft = super::Xn117Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_117_fenwick_point_query() {
        let mut ft = super::Xn117Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_117_fenwick_len() {
        let ft = super::Xn117Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_117_fenwick_multiple_updates() {
        let mut ft = super::Xn117Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_117_fenwick_single_element() {
        let mut ft = super::Xn117Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_117_fenwick_find_kth() {
        let mut ft = super::Xn117Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_117_fenwick_negative_delta() {
        let mut ft = super::Xn117Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 117 ----

    #[test]
    fn xn_117_avl_insert_get() {
        let mut m = super::Xn117AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_117_avl_remove() {
        let mut m = super::Xn117AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_117_avl_in_order() {
        let mut m = super::Xn117AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_117_avl_min_max() {
        let mut m = super::Xn117AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_117_avl_floor_ceiling() {
        let mut m = super::Xn117AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_117_avl_height_balanced() {
        let mut m = super::Xn117AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_117_avl_overwrite() {
        let mut m = super::Xn117AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_117_avl_empty() {
        let m: super::Xn117AVL<i32, i32> = super::Xn117AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo117RedBlack tests ---

    #[test]
    fn xo_117_rb_insert_and_get() {
        let mut tree = super::Xo117RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_117_rb_len_and_empty() {
        let mut tree = super::Xo117RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_117_rb_min_max() {
        let mut tree = super::Xo117RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_117_rb_contains() {
        let mut tree = super::Xo117RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_117_rb_remove() {
        let mut tree = super::Xo117RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_117_rb_in_order() {
        let mut tree = super::Xo117RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_117_rb_black_height() {
        let mut tree = super::Xo117RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_117_rb_overwrite() {
        let mut tree = super::Xo117RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo117ConsistentHash tests ---

    #[test]
    fn xo_117_ch_add_and_count() {
        let mut ring = super::Xo117ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_117_ch_remove_node() {
        let mut ring = super::Xo117ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_117_ch_get_node() {
        let mut ring = super::Xo117ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_117_ch_empty_ring() {
        let ring = super::Xo117ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_117_ch_distribution() {
        let mut ring = super::Xo117ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_117_ch_rebalance() {
        let mut ring = super::Xo117ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_117_ch_virtual_nodes() {
        let mut ring = super::Xo117ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_117_ch_consistent_lookup() {
        let mut ring = super::Xo117ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_117_splay_insert_get() {
        let mut t = super::Xp117SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_117_splay_remove() {
        let mut t = super::Xp117SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_117_splay_count_increases() {
        let mut t = super::Xp117SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_117_splay_depth() {
        let mut t = super::Xp117SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_117_splay_len_empty() {
        let t = super::Xp117SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_117_splay_min_max() {
        let mut t = super::Xp117SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_117_splay_overwrite() {
        let mut t = super::Xp117SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_117_splay_remove_missing() {
        let mut t = super::Xp117SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_117 treap tests ----
    #[test]
    fn xq_117_treap_empty() {
        let t = super::Xq117Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_117_treap_insert_get() {
        let mut t = super::Xq117Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_117_treap_overwrite() {
        let mut t = super::Xq117Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_117_treap_remove() {
        let mut t = super::Xq117Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_117_treap_min_max() {
        let mut t = super::Xq117Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_117_treap_rank() {
        let mut t = super::Xq117Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_117_treap_kth() {
        let mut t = super::Xq117Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_117_treap_in_order() {
        let mut t = super::Xq117Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_117 VEB tree tests ----
    #[test]
    fn xq_117_veb_empty() {
        let v = super::Xq117VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_117_veb_insert_contains() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_117_veb_min_max() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_117_veb_delete() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_117_veb_successor() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_117_veb_predecessor() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_117_veb_count() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_117_veb_duplicate_insert() {
        let mut v = super::Xq117VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}