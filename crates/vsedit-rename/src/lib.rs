//! Rename symbol support.
//!
//! Provides the rename workflow: prepare → validate → compute edits → apply.

use std::fmt;
use std::collections::HashMap;

/// A text edit for a rename operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEdit {
    pub uri: String,
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub new_text: String,
}

/// A location where a symbol being renamed occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameLocation {
    pub uri: String,
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub old_text: String,
}

impl RenameLocation {
    /// Convert this location into a rename edit with the given new name.
    pub fn to_edit(&self, new_name: &str) -> RenameEdit {
        RenameEdit {
            uri: self.uri.clone(),
            line: self.line,
            start_column: self.start_column,
            end_column: self.end_column,
            new_text: new_name.to_string(),
        }
    }
}

/// Range returned from prepare-rename indicating the symbol span and default name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameRange {
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub placeholder: String,
}

/// Result of preparing a rename — the range and placeholder text.
#[derive(Debug, Clone)]
pub struct PrepareRenameResult {
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub placeholder: String,
}

impl PrepareRenameResult {
    /// Convert to a `RenameRange`.
    pub fn to_range(&self) -> RenameRange {
        RenameRange {
            line: self.line,
            start_column: self.start_column,
            end_column: self.end_column,
            placeholder: self.placeholder.clone(),
        }
    }
}

/// A single text edit within a workspace edit (file-local coordinates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub new_text: String,
}

/// Result of performing a rename across the workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceEdit {
    pub edits: Vec<RenameEdit>,
    /// Multi-file changes keyed by URI.
    pub changes: HashMap<String, Vec<TextEdit>>,
}

impl WorkspaceEdit {
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            changes: HashMap::new(),
        }
    }

    /// Build a `WorkspaceEdit` from the `changes` map representation.
    pub fn from_changes(changes: HashMap<String, Vec<TextEdit>>) -> Self {
        Self {
            edits: Vec::new(),
            changes,
        }
    }

    /// Number of files affected by this edit (union of edits + changes).
    pub fn affected_file_count(&self) -> usize {
        let mut uris: Vec<&str> = self.edits.iter().map(|e| e.uri.as_str()).collect();
        for key in self.changes.keys() {
            uris.push(key.as_str());
        }
        uris.sort();
        uris.dedup();
        uris.len()
    }

    /// Total number of individual edits (flat edits + changes entries).
    pub fn edit_count(&self) -> usize {
        self.edits.len() + self.changes.values().map(|v| v.len()).sum::<usize>()
    }

    /// Insert a text edit for a specific file into the `changes` map.
    pub fn add_change(&mut self, uri: impl Into<String>, edit: TextEdit) {
        self.changes.entry(uri.into()).or_default().push(edit);
    }
}

impl Default for WorkspaceEdit {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider trait for rename operations.
pub trait RenameProvider: Send + Sync {
    fn prepare_rename(&self, uri: &str, line: u32, column: u32) -> Option<PrepareRenameResult>;
    fn provide_rename_edits(&self, uri: &str, line: u32, column: u32, new_name: &str) -> Option<WorkspaceEdit>;
}

/// Service that orchestrates the rename workflow.
pub struct RenameService {
    providers: Vec<Box<dyn RenameProvider>>,
}

impl RenameService {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn RenameProvider>) {
        self.providers.push(provider);
    }

    /// Prepare rename: validate rename is possible and get default name / range.
    pub fn prepare_rename(&self, uri: &str, line: u32, column: u32) -> Option<RenameRange> {
        for provider in &self.providers {
            if let Some(result) = provider.prepare_rename(uri, line, column) {
                return Some(result.to_range());
            }
        }
        None
    }

    /// Legacy alias for `prepare_rename`.
    pub fn prepare(&self, uri: &str, line: u32, column: u32) -> Option<PrepareRenameResult> {
        for provider in &self.providers {
            if let Some(result) = provider.prepare_rename(uri, line, column) {
                return Some(result);
            }
        }
        None
    }

    /// Execute rename: compute all edits for the given new name.
    pub fn execute_rename(
        &self,
        uri: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<WorkspaceEdit, RenameError> {
        Self::validate_new_name(new_name)?;
        for provider in &self.providers {
            if let Some(edit) = provider.provide_rename_edits(uri, line, column, new_name) {
                return Ok(edit);
            }
        }
        Err(RenameError::NoProvider)
    }

    /// Validate the new name (non-empty, no whitespace-only, etc.).
    pub fn validate_new_name(name: &str) -> Result<(), RenameError> {
        if name.is_empty() {
            return Err(RenameError::EmptyName);
        }
        if name.trim().is_empty() {
            return Err(RenameError::WhitespaceOnly);
        }
        if name.contains('\n') || name.contains('\r') {
            return Err(RenameError::ContainsNewline);
        }
        Ok(())
    }

    /// Compute edits for the rename (alias for `execute_rename`).
    pub fn compute_edits(
        &self,
        uri: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<WorkspaceEdit, RenameError> {
        self.execute_rename(uri, line, column, new_name)
    }
}

impl Default for RenameService {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a workspace edit atomically. Returns per-file results.
/// Files are represented as `(uri, content)` pairs; the function returns
/// the transformed contents keyed by URI.
pub fn apply_workspace_edit(
    files: &HashMap<String, String>,
    edit: &WorkspaceEdit,
) -> Result<HashMap<String, String>, RenameError> {
    let mut result = files.clone();

    // Apply flat edits grouped by file (reverse order to preserve positions).
    let mut by_file: HashMap<&str, Vec<&RenameEdit>> = HashMap::new();
    for e in &edit.edits {
        by_file.entry(e.uri.as_str()).or_default().push(e);
    }
    for (uri, mut edits) in by_file {
        let content = result
            .get(uri)
            .ok_or_else(|| RenameError::FileNotFound(uri.to_string()))?;
        edits.sort_by(|a, b| b.line.cmp(&a.line).then(b.start_column.cmp(&a.start_column)));
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for e in edits {
            let idx = e.line as usize;
            if idx < lines.len() {
                let line = &lines[idx];
                let s = e.start_column as usize;
                let end = e.end_column as usize;
                let new_line = format!(
                    "{}{}{}",
                    &line[..s.min(line.len())],
                    e.new_text,
                    &line[end.min(line.len())..]
                );
                lines[idx] = new_line;
            }
        }
        result.insert(uri.to_string(), lines.join("\n"));
    }

    // Apply changes map.
    for (uri, text_edits) in &edit.changes {
        let content = result
            .get(uri.as_str())
            .ok_or_else(|| RenameError::FileNotFound(uri.clone()))?;
        let mut sorted: Vec<&TextEdit> = text_edits.iter().collect();
        sorted.sort_by(|a, b| {
            b.start_line
                .cmp(&a.start_line)
                .then(b.start_column.cmp(&a.start_column))
        });
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for te in sorted {
            if te.start_line == te.end_line {
                let idx = te.start_line as usize;
                if idx < lines.len() {
                    let line = &lines[idx];
                    let s = te.start_column as usize;
                    let e = te.end_column as usize;
                    lines[idx] = format!(
                        "{}{}{}",
                        &line[..s.min(line.len())],
                        te.new_text,
                        &line[e.min(line.len())..]
                    );
                }
            }
        }
        result.insert(uri.clone(), lines.join("\n"));
    }

    Ok(result)
}

/// Preview of changes a rename would produce.
#[derive(Debug, Clone)]
pub struct RenamePreview {
    /// Mapping of uri → list of (line_number, old_line, new_line).
    pub file_changes: HashMap<String, Vec<(u32, String, String)>>,
}

impl RenamePreview {
    /// Build a preview from a workspace edit and existing file contents.
    pub fn from_edit(files: &HashMap<String, String>, edit: &WorkspaceEdit) -> Self {
        let mut file_changes: HashMap<String, Vec<(u32, String, String)>> = HashMap::new();

        // Collect all edits by file.
        let mut by_file: HashMap<String, Vec<(u32, u32, u32, &str)>> = HashMap::new();
        for e in &edit.edits {
            by_file
                .entry(e.uri.clone())
                .or_default()
                .push((e.line, e.start_column, e.end_column, e.new_text.as_str()));
        }
        for (uri, text_edits) in &edit.changes {
            for te in text_edits {
                if te.start_line == te.end_line {
                    by_file
                        .entry(uri.clone())
                        .or_default()
                        .push((te.start_line, te.start_column, te.end_column, te.new_text.as_str()));
                }
            }
        }

        for (uri, edits) in &by_file {
            if let Some(content) = files.get(uri.as_str()) {
                let lines: Vec<&str> = content.lines().collect();
                let mut changes = Vec::new();
                for &(line, sc, ec, new_text) in edits {
                    let idx = line as usize;
                    if idx < lines.len() {
                        let old_line = lines[idx].to_string();
                        let s = sc as usize;
                        let e = ec as usize;
                        let new_line = format!(
                            "{}{}{}",
                            &old_line[..s.min(old_line.len())],
                            new_text,
                            &old_line[e.min(old_line.len())..]
                        );
                        changes.push((line, old_line, new_line));
                    }
                }
                file_changes.insert(uri.clone(), changes);
            }
        }

        Self { file_changes }
    }

    /// Total number of changed lines across all files.
    pub fn total_changes(&self) -> usize {
        self.file_changes.values().map(|v| v.len()).sum()
    }

    /// Number of files affected.
    pub fn file_count(&self) -> usize {
        self.file_changes.len()
    }
}

/// Errors that can occur during rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    EmptyName,
    WhitespaceOnly,
    ContainsNewline,
    NoProvider,
    /// A file referenced in the edit was not found.
    FileNotFound(String),
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "New name cannot be empty"),
            Self::WhitespaceOnly => write!(f, "New name cannot be whitespace only"),
            Self::ContainsNewline => write!(f, "New name cannot contain newlines"),
            Self::NoProvider => write!(f, "No rename provider available"),
            Self::FileNotFound(uri) => write!(f, "File not found: {}", uri),
        }
    }
}

/// State for the rename input widget (inline rename UI).
#[derive(Debug, Clone)]
pub struct RenameInputWidget {
    /// Line where the rename is happening.
    pub line: u32,
    /// Column where the rename input is placed.
    pub column: u32,
    /// Original name being renamed.
    pub old_name: String,
    /// Current text in the input box.
    pub new_name: String,
    /// Cursor position within the input.
    pub cursor_pos: usize,
    /// Whether the current input is valid.
    pub is_valid: bool,
}

impl RenameInputWidget {
    pub fn new(line: u32, column: u32, old_name: impl Into<String>) -> Self {
        let old = old_name.into();
        let len = old.len();
        Self {
            line,
            column,
            old_name: old.clone(),
            new_name: old,
            cursor_pos: len,
            is_valid: true,
        }
    }

    /// Update the new name and revalidate.
    pub fn set_new_name(&mut self, name: impl Into<String>) {
        self.new_name = name.into();
        self.is_valid = RenameService::validate_new_name(&self.new_name).is_ok();
    }

    /// Whether the name has actually changed.
    pub fn has_changed(&self) -> bool {
        self.new_name != self.old_name
    }

    /// Accept the rename (returns new name if valid and changed).
    pub fn accept(&self) -> Option<&str> {
        if self.is_valid && self.has_changed() {
            Some(&self.new_name)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// RenameEdit helpers
// ---------------------------------------------------------------------------

impl RenameEdit {
    /// Span length of the replaced text.
    pub fn span_length(&self) -> u32 {
        self.end_column.saturating_sub(self.start_column)
    }

    /// Whether this edit replaces text in a given file URI.
    pub fn is_in_file(&self, uri: &str) -> bool {
        self.uri == uri
    }

    /// Net character delta introduced by this edit (new length minus old span).
    pub fn char_delta(&self) -> i64 {
        self.new_text.len() as i64 - self.span_length() as i64
    }
}

impl std::fmt::Display for RenameEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}-{} -> \"{}\"",
            self.uri, self.line, self.start_column, self.end_column, self.new_text
        )
    }
}

// ---------------------------------------------------------------------------
// RenameLocation helpers
// ---------------------------------------------------------------------------

impl RenameLocation {
    /// Span length of the located text.
    pub fn span_length(&self) -> u32 {
        self.end_column.saturating_sub(self.start_column)
    }

    /// Whether the location is on a particular line.
    pub fn is_on_line(&self, line: u32) -> bool {
        self.line == line
    }
}

impl std::fmt::Display for RenameLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}-{} \"{}\"",
            self.uri, self.line, self.start_column, self.end_column, self.old_text
        )
    }
}

// ---------------------------------------------------------------------------
// WorkspaceEdit helpers
// ---------------------------------------------------------------------------

impl WorkspaceEdit {
    /// Filter edits to only those touching a specific file.
    pub fn edits_for_file(&self, uri: &str) -> Vec<&RenameEdit> {
        self.edits.iter().filter(|e| e.uri == uri).collect()
    }

    /// Merge another WorkspaceEdit into this one.
    pub fn merge(&mut self, other: WorkspaceEdit) {
        self.edits.extend(other.edits);
        for (uri, edits) in other.changes {
            self.changes.entry(uri).or_default().extend(edits);
        }
    }

    /// Sort edits by (uri, line, start_column) for deterministic application.
    pub fn sort_edits(&mut self) {
        self.edits
            .sort_by(|a, b| (&a.uri, a.line, a.start_column).cmp(&(&b.uri, b.line, b.start_column)));
    }

    /// Return all unique file URIs touched by these edits (flat + changes).
    pub fn affected_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.edits.iter().map(|e| e.uri.clone()).collect();
        for key in self.changes.keys() {
            uris.push(key.clone());
        }
        uris.sort();
        uris.dedup();
        uris
    }
}

impl std::fmt::Display for WorkspaceEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorkspaceEdit({} edits in {} files)", self.edit_count(), self.affected_file_count())
    }
}

// ---------------------------------------------------------------------------
// Name validation utilities
// ---------------------------------------------------------------------------

/// Check whether a candidate name is a valid identifier (ASCII alphanumeric + underscore,
/// not starting with a digit).
pub fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Classify how a name was changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameKind {
    /// Only casing changed (e.g. `foo` → `Foo`).
    CaseChange,
    /// Completely different name.
    FullRename,
    /// Prefix/suffix added.
    Augmented,
}

/// Determine the kind of rename from old to new name.
pub fn classify_rename(old: &str, new: &str) -> RenameKind {
    if old.eq_ignore_ascii_case(new) {
        RenameKind::CaseChange
    } else if new.contains(old) || old.contains(new) {
        RenameKind::Augmented
    } else {
        RenameKind::FullRename
    }
}

// ---------------------------------------------------------------------------
// RenameHistory — undo/redo support
// ---------------------------------------------------------------------------

/// Tracks performed renames for undo/redo.
#[derive(Debug, Clone)]
pub struct RenameHistoryEntry {
    pub old_name: String,
    pub new_name: String,
    pub edit: WorkspaceEdit,
}

/// Maintains a stack of rename operations.
#[derive(Debug, Clone)]
pub struct RenameHistory {
    entries: Vec<RenameHistoryEntry>,
    max_entries: usize,
}

impl RenameHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn push(&mut self, entry: RenameHistoryEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn pop(&mut self) -> Option<RenameHistoryEntry> {
        self.entries.pop()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Peek at the most recent entry without removing it.
    pub fn last(&self) -> Option<&RenameHistoryEntry> {
        self.entries.last()
    }
}

impl Default for RenameHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

// ---------------------------------------------------------------------------
// Impact analysis
// ---------------------------------------------------------------------------

/// A single location affected by a rename, used for impact analysis display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactLocation {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub context: String,
}

/// Summary of the impact a rename workspace edit would have.
#[derive(Debug, Clone)]
pub struct RenameImpactAnalysis {
    pub files_affected: usize,
    pub total_edits: usize,
    pub locations: Vec<ImpactLocation>,
}

impl RenameImpactAnalysis {
    /// Build an impact analysis from a `WorkspaceEdit`.
    pub fn from_workspace_edit(edit: &WorkspaceEdit) -> Self {
        let mut locations = Vec::new();

        // Collect from flat edits.
        for e in &edit.edits {
            locations.push(ImpactLocation {
                file_path: e.uri.clone(),
                line: e.line,
                column: e.start_column,
                length: e.end_column.saturating_sub(e.start_column),
                context: format!(
                    "{}:{}:{}-{} -> \"{}\"",
                    e.uri, e.line, e.start_column, e.end_column, e.new_text
                ),
            });
        }

        // Collect from changes map.
        for (uri, text_edits) in &edit.changes {
            for te in text_edits {
                locations.push(ImpactLocation {
                    file_path: uri.clone(),
                    line: te.start_line,
                    column: te.start_column,
                    length: te.end_column.saturating_sub(te.start_column),
                    context: format!(
                        "{}:{}:{}-{} -> \"{}\"",
                        uri, te.start_line, te.start_column, te.end_column, te.new_text
                    ),
                });
            }
        }

        let files_affected = edit.affected_file_count();
        let total_edits = locations.len();

        Self {
            files_affected,
            total_edits,
            locations,
        }
    }

    /// Unique file paths affected.
    pub fn files_list(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.locations.iter().map(|l| l.file_path.as_str()).collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// Count of edits in a specific file.
    pub fn edits_in_file(&self, path: &str) -> usize {
        self.locations.iter().filter(|l| l.file_path == path).count()
    }
}

/// Validate that a rename from `old_name` to `new_name` is acceptable.
///
/// Returns `Ok(())` when valid, or `Err` with a list of validation failures.
pub fn rename_validate(old_name: &str, new_name: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if new_name.is_empty() {
        errors.push("new name must not be empty".to_string());
    }

    if new_name == old_name {
        errors.push("new name must differ from old name".to_string());
    }

    if new_name.contains('/') || new_name.contains('\\') {
        errors.push("new name must not contain path separators".to_string());
    }

    if new_name != new_name.trim() {
        errors.push("new name must not start or end with whitespace".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// RenameConflictDetector – detect naming conflicts
// ---------------------------------------------------------------------------

/// A detected naming conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameConflict {
    /// File where the conflict exists.
    pub file_path: String,
    /// Line number of the conflicting symbol.
    pub line: u32,
    /// The existing name that conflicts.
    pub existing_name: String,
}

/// Detects naming conflicts when renaming a symbol.
pub struct RenameConflictDetector;

impl RenameConflictDetector {
    /// Check whether `new_name` already exists in any of the provided symbol lists.
    /// `symbols_by_file` maps file path → list of (line, name) pairs.
    pub fn detect(
        new_name: &str,
        symbols_by_file: &HashMap<String, Vec<(u32, String)>>,
    ) -> Vec<RenameConflict> {
        let mut conflicts = Vec::new();
        for (file, symbols) in symbols_by_file {
            for (line, name) in symbols {
                if name == new_name {
                    conflicts.push(RenameConflict {
                        file_path: file.clone(),
                        line: *line,
                        existing_name: name.clone(),
                    });
                }
            }
        }
        conflicts.sort_by(|a, b| (&a.file_path, a.line).cmp(&(&b.file_path, b.line)));
        conflicts
    }

    /// Quick check if there is at least one conflict.
    pub fn has_conflict(
        new_name: &str,
        symbols_by_file: &HashMap<String, Vec<(u32, String)>>,
    ) -> bool {
        symbols_by_file
            .values()
            .any(|syms| syms.iter().any(|(_, n)| n == new_name))
    }
}

// ---------------------------------------------------------------------------
// RenameSuggestions – suggest new names based on patterns
// ---------------------------------------------------------------------------

/// Generates name suggestions based on common patterns.
pub struct RenameSuggestions;

impl RenameSuggestions {
    /// Suggest variations of `name` using common naming conventions.
    pub fn suggest(name: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        // camelCase → snake_case
        let snake = Self::to_snake_case(name);
        if snake != name {
            suggestions.push(snake);
        }

        // snake_case → camelCase
        let camel = Self::to_camel_case(name);
        if camel != name {
            suggestions.push(camel);
        }

        // UPPER_CASE
        let upper = name.to_ascii_uppercase();
        if upper != name {
            suggestions.push(upper);
        }

        // Prefix with underscore (private convention)
        if !name.starts_with('_') {
            suggestions.push(format!("_{name}"));
        }

        suggestions
    }

    fn to_snake_case(name: &str) -> String {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if ch.is_ascii_uppercase() && i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        }
        result
    }

    fn to_camel_case(name: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = false;
        for ch in name.chars() {
            if ch == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// RenameScope – limit rename to specific scopes
// ---------------------------------------------------------------------------

/// Defines the scope for a rename operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameScope {
    /// Rename in the entire workspace.
    Workspace,
    /// Rename only in a single file.
    SingleFile(String),
    /// Rename in specific files.
    Files(Vec<String>),
    /// Rename only within a line range in a single file.
    Range {
        file: String,
        start_line: u32,
        end_line: u32,
    },
}

impl RenameScope {
    /// Check if a given file URI is within scope.
    pub fn includes_file(&self, uri: &str) -> bool {
        match self {
            RenameScope::Workspace => true,
            RenameScope::SingleFile(f) => f == uri,
            RenameScope::Files(files) => files.iter().any(|f| f == uri),
            RenameScope::Range { file, .. } => file == uri,
        }
    }

    /// Filter a workspace edit to only include edits within scope.
    pub fn filter_edit(&self, edit: &WorkspaceEdit) -> WorkspaceEdit {
        let filtered_edits: Vec<RenameEdit> = edit
            .edits
            .iter()
            .filter(|e| self.includes_edit(e))
            .cloned()
            .collect();
        let filtered_changes: HashMap<String, Vec<TextEdit>> = edit
            .changes
            .iter()
            .filter(|(uri, _)| self.includes_file(uri))
            .map(|(uri, edits)| (uri.clone(), edits.clone()))
            .collect();
        WorkspaceEdit {
            edits: filtered_edits,
            changes: filtered_changes,
        }
    }

    fn includes_edit(&self, edit: &RenameEdit) -> bool {
        if !self.includes_file(&edit.uri) {
            return false;
        }
        if let RenameScope::Range { start_line, end_line, .. } = self {
            edit.line >= *start_line && edit.line <= *end_line
        } else {
            true
        }
    }
}

// ---------------------------------------------------------------------------
// RenameHistory diff/comparison extension
// ---------------------------------------------------------------------------

impl RenameHistory {
    /// Return a summary of all renames performed, in order.
    pub fn summary(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| format!("{} → {}", e.old_name, e.new_name))
            .collect()
    }

    /// Find all rename entries that touched a specific file.
    pub fn entries_for_file(&self, uri: &str) -> Vec<&RenameHistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.edit.edits.iter().any(|ed| ed.uri == uri))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// RenameRefactorPlan — plan multi-step renames
// ---------------------------------------------------------------------------

/// A planned rename step with an ordering priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameStep {
    pub old_name: String,
    pub new_name: String,
    pub scope: RenameScope,
    pub priority: u32,
}

/// A multi-step rename plan that can be validated and executed in order.
#[derive(Debug, Clone, Default)]
pub struct RenameRefactorPlan {
    steps: Vec<RenameStep>,
}

impl RenameRefactorPlan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rename step to the plan.
    pub fn add_step(&mut self, old_name: &str, new_name: &str, scope: RenameScope, priority: u32) {
        self.steps.push(RenameStep {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
            scope,
            priority,
        });
    }

    /// Return steps sorted by priority (lower first).
    pub fn ordered_steps(&self) -> Vec<&RenameStep> {
        let mut sorted: Vec<&RenameStep> = self.steps.iter().collect();
        sorted.sort_by_key(|s| s.priority);
        sorted
    }

    /// Validate all steps in the plan. Returns a list of (step_index, errors) for invalid steps.
    pub fn validate(&self) -> Vec<(usize, Vec<String>)> {
        let mut issues = Vec::new();
        for (i, step) in self.steps.iter().enumerate() {
            if let Err(errors) = rename_validate(&step.old_name, &step.new_name) {
                issues.push((i, errors));
            }
        }
        issues
    }

    /// Check for chained renames (output of one step is input of a later step).
    pub fn chained_steps(&self) -> Vec<(usize, usize)> {
        let mut chains = Vec::new();
        for (i, a) in self.steps.iter().enumerate() {
            for (j, b) in self.steps.iter().enumerate() {
                if i != j && a.new_name == b.old_name {
                    chains.push((i, j));
                }
            }
        }
        chains
    }

    /// Number of steps in the plan.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Return all unique file URIs affected by SingleFile or Files scopes.
    pub fn affected_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self.steps.iter().filter_map(|s| {
            match &s.scope {
                RenameScope::SingleFile(f) => Some(f.as_str()),
                RenameScope::Range { file, .. } => Some(file.as_str()),
                _ => None,
            }
        }).collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    /// Remove a step by index. Returns the removed step or None.
    pub fn remove_step(&mut self, index: usize) -> Option<RenameStep> {
        if index < self.steps.len() {
            Some(self.steps.remove(index))
        } else {
            None
        }
    }
}

impl std::fmt::Display for RenameStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} → {} (priority {})", self.old_name, self.new_name, self.priority)
    }
}

impl std::fmt::Display for RenameRefactorPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Rename plan ({} steps):", self.steps.len())?;
        for (i, step) in self.ordered_steps().iter().enumerate() {
            writeln!(f, "  {}. {}", i + 1, step)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Case transformation helpers
// ---------------------------------------------------------------------------

/// Convert a name to `snake_case`.
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            let prev = name.as_bytes()[i - 1] as char;
            if prev != '_' && !prev.is_ascii_uppercase() {
                result.push('_');
            } else if prev.is_ascii_uppercase() {
                // Handle sequences like "HTMLParser" -> "html_parser"
                if let Some(&next) = name.as_bytes().get(i + 1) {
                    if (next as char).is_ascii_lowercase() {
                        result.push('_');
                    }
                }
            }
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

/// Convert a name to `camelCase`.
pub fn to_camel_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut capitalize_next = false;
    for (i, ch) in name.chars().enumerate() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert a name to `PascalCase`.
pub fn to_pascal_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut capitalize_next = true;
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert a name to `SCREAMING_SNAKE_CASE`.
pub fn to_screaming_snake_case(name: &str) -> String {
    to_snake_case(name).to_ascii_uppercase()
}

/// Convert a name to `kebab-case`.
pub fn to_kebab_case(name: &str) -> String {
    to_snake_case(name).replace('_', "-")
}

// ---------------------------------------------------------------------------
// Batch rename with pattern matching
// ---------------------------------------------------------------------------

/// A batch rename operation that applies a find/replace pattern across files.
#[derive(Debug, Clone)]
pub struct BatchRename {
    /// The pattern to search for (literal substring).
    pub pattern: String,
    /// The replacement string.
    pub replacement: String,
}

impl BatchRename {
    pub fn new(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
        }
    }

    /// Scan file contents and produce a `WorkspaceEdit` for all occurrences.
    /// `files` maps URI → file content.
    pub fn compute_edits(&self, files: &HashMap<String, String>) -> WorkspaceEdit {
        let mut we = WorkspaceEdit::new();
        for (uri, content) in files {
            for (line_idx, line) in content.lines().enumerate() {
                let mut search_start = 0;
                while let Some(col) = line[search_start..].find(&self.pattern) {
                    let abs_col = search_start + col;
                    we.edits.push(RenameEdit {
                        uri: uri.clone(),
                        line: line_idx as u32,
                        start_column: abs_col as u32,
                        end_column: (abs_col + self.pattern.len()) as u32,
                        new_text: self.replacement.clone(),
                    });
                    search_start = abs_col + self.pattern.len();
                }
            }
        }
        we.sort_edits();
        we
    }

    /// Count total occurrences across all files without building edits.
    pub fn count_occurrences(&self, files: &HashMap<String, String>) -> usize {
        files
            .values()
            .map(|content| content.matches(&self.pattern).count())
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Rename undo preparation
// ---------------------------------------------------------------------------

/// Captures the state needed to undo a rename operation.
#[derive(Debug, Clone)]
pub struct RenameUndoInfo {
    /// The original file contents before the rename, keyed by URI.
    pub original_contents: HashMap<String, String>,
    /// The old symbol name.
    pub old_name: String,
    /// The new symbol name that was applied.
    pub new_name: String,
}

impl RenameUndoInfo {
    /// Prepare undo info by snapshotting the files that will be affected.
    pub fn prepare(
        files: &HashMap<String, String>,
        edit: &WorkspaceEdit,
        old_name: &str,
        new_name: &str,
    ) -> Self {
        let affected = edit.affected_uris();
        let original_contents = files
            .iter()
            .filter(|(uri, _)| affected.contains(uri))
            .map(|(uri, content)| (uri.clone(), content.clone()))
            .collect();
        Self {
            original_contents,
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
        }
    }

    /// Number of files captured for undo.
    pub fn file_count(&self) -> usize {
        self.original_contents.len()
    }

    /// Restore original contents into the provided mutable file map.
    pub fn restore_into(&self, files: &mut HashMap<String, String>) {
        for (uri, content) in &self.original_contents {
            files.insert(uri.clone(), content.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Multi-file rename tracker
// ---------------------------------------------------------------------------

/// Tracks the progress of a rename operation across multiple files.
#[derive(Debug, Clone)]
pub struct RenameProgress {
    /// Files that still need processing.
    pub pending: Vec<String>,
    /// Files that have been successfully renamed.
    pub completed: Vec<String>,
    /// Files that failed with an error message.
    pub failed: Vec<(String, String)>,
}

impl RenameProgress {
    /// Create a new tracker from a list of file URIs.
    pub fn new(files: Vec<String>) -> Self {
        Self {
            pending: files,
            completed: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Mark a file as successfully processed.
    pub fn mark_completed(&mut self, uri: &str) {
        self.pending.retain(|f| f != uri);
        self.completed.push(uri.to_string());
    }

    /// Mark a file as failed.
    pub fn mark_failed(&mut self, uri: &str, reason: &str) {
        self.pending.retain(|f| f != uri);
        self.failed.push((uri.to_string(), reason.to_string()));
    }

    /// Whether all files have been processed (completed or failed).
    pub fn is_done(&self) -> bool {
        self.pending.is_empty()
    }

    /// Whether any files failed.
    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }

    /// Fraction of work completed (0.0 to 1.0).
    pub fn progress_fraction(&self) -> f64 {
        let total = self.pending.len() + self.completed.len() + self.failed.len();
        if total == 0 {
            return 1.0;
        }
        (self.completed.len() + self.failed.len()) as f64 / total as f64
    }
}

// ---------------------------------------------------------------------------
// Rename validation for common language identifiers
// ---------------------------------------------------------------------------

/// Detailed validation result for a proposed rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameValidation {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Validate a rename with detailed diagnostics including warnings.
pub fn validate_rename_detailed(old_name: &str, new_name: &str) -> RenameValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if new_name.is_empty() {
        errors.push("new name must not be empty".to_string());
    }
    if new_name == old_name {
        errors.push("new name is identical to old name".to_string());
    }
    if !new_name.is_empty() && !is_valid_identifier(new_name) {
        errors.push("new name is not a valid identifier".to_string());
    }

    // Warnings (non-blocking)
    if new_name.starts_with('_') && !old_name.starts_with('_') {
        warnings.push("new name starts with underscore (convention for unused)".to_string());
    }
    if new_name.len() == 1 {
        warnings.push("single-character names reduce readability".to_string());
    }
    if new_name.len() > 64 {
        warnings.push("name is unusually long (>64 chars)".to_string());
    }

    // Check for mixed naming conventions
    let has_underscore = new_name.contains('_');
    let has_uppercase = new_name.chars().any(|c| c.is_ascii_uppercase());
    if has_underscore && has_uppercase && !new_name.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()) {
        warnings.push("name mixes snake_case and camelCase conventions".to_string());
    }

    RenameValidation {
        is_valid: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Check if a set of reserved/keyword names would conflict with the new name.
pub fn is_reserved_name(name: &str, reserved: &[&str]) -> bool {
    reserved.iter().any(|&r| r == name)
}

/// Common Rust keywords that should not be used as identifier names.
pub const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
    "trait", "true", "type", "unsafe", "use", "where", "while", "yield",
];

// ---------------------------------------------------------------------------
// RenamePreviewGenerator
// ---------------------------------------------------------------------------

/// A single change in a rename preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePreviewChange {
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub old_text: String,
    pub new_text: String,
}

impl RenamePreviewChange {
    pub fn new(
        file_path: impl Into<String>,
        line: usize,
        column: usize,
        old_text: impl Into<String>,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            line,
            column,
            old_text: old_text.into(),
            new_text: new_text.into(),
        }
    }

    /// Returns a unified-diff-style summary line.
    pub fn diff_line(&self) -> String {
        format!("{}:{}:{}: '{}' -> '{}'", self.file_path, self.line, self.column, self.old_text, self.new_text)
    }
}

impl std::fmt::Display for RenamePreviewChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diff_line())
    }
}

/// Generates a preview of rename changes across files.
pub struct RenamePreviewGenerator {
    changes: Vec<RenamePreviewChange>,
}

impl RenamePreviewGenerator {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    pub fn add_change(&mut self, change: RenamePreviewChange) {
        self.changes.push(change);
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// How many distinct files are affected.
    pub fn affected_file_count(&self) -> usize {
        let mut files = std::collections::HashSet::new();
        for c in &self.changes {
            files.insert(&c.file_path);
        }
        files.len()
    }

    /// Return changes grouped by file.
    pub fn changes_by_file(&self) -> std::collections::HashMap<&str, Vec<&RenamePreviewChange>> {
        let mut map: std::collections::HashMap<&str, Vec<&RenamePreviewChange>> =
            std::collections::HashMap::new();
        for c in &self.changes {
            map.entry(&c.file_path).or_default().push(c);
        }
        map
    }

    /// Generate a full preview string.
    pub fn generate_preview(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Rename Preview: {} changes in {} files",
            self.changes.len(), self.affected_file_count()));
        lines.push("---".into());
        let by_file = self.changes_by_file();
        let mut files: Vec<&&str> = by_file.keys().collect();
        files.sort();
        for file in files {
            lines.push(format!("  {file}:"));
            for change in &by_file[*file] {
                lines.push(format!("    L{}:C{}: '{}' -> '{}'",
                    change.line, change.column, change.old_text, change.new_text));
            }
        }
        lines.join("\n")
    }

    /// Clear all changes.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// List affected files sorted alphabetically.
    pub fn affected_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self
            .changes
            .iter()
            .map(|c| c.file_path.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        files.sort();
        files
    }
}

impl std::fmt::Display for RenamePreviewGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RenamePreviewGenerator({} changes, {} files)",
            self.changes.len(), self.affected_file_count())
    }
}

// ---------------------------------------------------------------------------
// RenameConflictResolver
// ---------------------------------------------------------------------------

/// The kind of naming conflict detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameConflictKind {
    /// New name already exists in the same scope.
    NameAlreadyExists,
    /// New name shadows a name from an outer scope.
    ShadowsOuterScope,
    /// New name is a language keyword.
    ReservedKeyword,
    /// New name collides with an import.
    ImportCollision,
}

impl std::fmt::Display for RenameConflictKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenameConflictKind::NameAlreadyExists => write!(f, "name already exists"),
            RenameConflictKind::ShadowsOuterScope => write!(f, "shadows outer scope"),
            RenameConflictKind::ReservedKeyword => write!(f, "reserved keyword"),
            RenameConflictKind::ImportCollision => write!(f, "import collision"),
        }
    }
}

/// A detected naming conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedRenameConflict {
    pub new_name: String,
    pub file_path: String,
    pub line: usize,
    pub kind: RenameConflictKind,
    pub suggestion: Option<String>,
}

impl DetectedRenameConflict {
    pub fn new(
        new_name: impl Into<String>,
        file_path: impl Into<String>,
        line: usize,
        kind: RenameConflictKind,
    ) -> Self {
        Self {
            new_name: new_name.into(),
            file_path: file_path.into(),
            line,
            kind,
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for DetectedRenameConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Conflict: '{}' at {}:{} - {}", self.new_name, self.file_path, self.line, self.kind)?;
        if let Some(s) = &self.suggestion {
            write!(f, " (suggest: {s})")?;
        }
        Ok(())
    }
}

/// Detects and resolves naming conflicts during rename operations.
pub struct RenameConflictResolver {
    existing_names: std::collections::HashSet<String>,
    keywords: std::collections::HashSet<String>,
    imports: std::collections::HashSet<String>,
}

impl RenameConflictResolver {
    pub fn new() -> Self {
        Self {
            existing_names: std::collections::HashSet::new(),
            keywords: std::collections::HashSet::new(),
            imports: std::collections::HashSet::new(),
        }
    }

    pub fn add_existing_name(&mut self, name: impl Into<String>) {
        self.existing_names.insert(name.into());
    }

    pub fn add_keyword(&mut self, kw: impl Into<String>) {
        self.keywords.insert(kw.into());
    }

    pub fn add_import(&mut self, import: impl Into<String>) {
        self.imports.insert(import.into());
    }

    /// Check a proposed new name for conflicts at a given location.
    pub fn check(&self, new_name: &str, file_path: &str, line: usize) -> Vec<DetectedRenameConflict> {
        let mut conflicts = Vec::new();

        if self.keywords.contains(new_name) {
            conflicts.push(
                DetectedRenameConflict::new(new_name, file_path, line, RenameConflictKind::ReservedKeyword)
                    .with_suggestion(format!("r#{new_name}")),
            );
        }

        if self.existing_names.contains(new_name) {
            conflicts.push(
                DetectedRenameConflict::new(new_name, file_path, line, RenameConflictKind::NameAlreadyExists)
                    .with_suggestion(format!("{new_name}_1")),
            );
        }

        if self.imports.contains(new_name) {
            conflicts.push(
                DetectedRenameConflict::new(new_name, file_path, line, RenameConflictKind::ImportCollision),
            );
        }

        conflicts
    }

    /// Suggest an alternative name that doesn't conflict.
    pub fn suggest_alternative(&self, base_name: &str) -> String {
        let mut candidate = base_name.to_string();
        let mut suffix = 1u32;
        while self.existing_names.contains(&candidate)
            || self.keywords.contains(&candidate)
            || self.imports.contains(&candidate)
        {
            candidate = format!("{base_name}_{suffix}");
            suffix += 1;
        }
        candidate
    }

    /// Number of registered existing names.
    pub fn existing_name_count(&self) -> usize {
        self.existing_names.len()
    }

    pub fn clear(&mut self) {
        self.existing_names.clear();
        self.keywords.clear();
        self.imports.clear();
    }
}

impl std::fmt::Display for RenameConflictResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RenameConflictResolver(names={}, kw={}, imports={})",
            self.existing_names.len(), self.keywords.len(), self.imports.len())
    }
}



// ---------------------------------------------------------------------------
// rename – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XRenameLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XRenamePanelState {
    pub region: XRenameLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XRenamePanelState {
    pub fn new(region: XRenameLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_rename_total_visible_area(panels: &[XRenamePanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_rename_count_in_region(
    panels: &[XRenamePanelState],
    region: XRenameLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_rename_widest_panel(panels: &[XRenamePanelState]) -> Option<&XRenamePanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_rename_collapse_region(
    panels: &mut [XRenamePanelState],
    region: XRenameLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XRenameLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XRenameLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// rename – Extended rename audit trail helpers
// ---------------------------------------------------------------------------

/// Priority levels for rename audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZRenamePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZRenamePriority {
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
    pub fn all_asc() -> [ZRenamePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZRenamePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks rename audit trail data.
#[derive(Debug, Clone)]
pub struct ZRenameRenameAuditTrail {
    pub records: Vec<(String, String)>,
    pub user_id: String,
    pub approved: bool,
}

impl ZRenameRenameAuditTrail {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            user_id: String::new(),
            approved: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZRenameRenameAuditTrail[user_id={:?}, approved={:?}]", self.user_id, self.approved)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.approved = !c.approved;
        c
    }
}

/// Compute a simple rolling hash for rename audit trail.
pub fn z_rename_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_rename_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_rename_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_rename_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_rename_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_rename_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_rename_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
// xb_ utilities – batch 36
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer36 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer36 {
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
pub fn xb_fnv1a_36(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_36<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_36<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_36(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_36(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_edit_creation() {
        let edit = RenameEdit {
            uri: "file:///test.rs".into(),
            line: 1,
            start_column: 5,
            end_column: 10,
            new_text: "newName".into(),
        };
        assert_eq!(edit.new_text, "newName");
    }

    #[test]
    fn workspace_edit_empty() {
        let we = WorkspaceEdit::new();
        assert!(we.edits.is_empty());
        assert_eq!(we.affected_file_count(), 0);
        assert_eq!(we.edit_count(), 0);
    }

    #[test]
    fn workspace_edit_affected_files() {
        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 3, new_text: "x".into() },
                RenameEdit { uri: "b.rs".into(), line: 2, start_column: 0, end_column: 3, new_text: "x".into() },
                RenameEdit { uri: "a.rs".into(), line: 5, start_column: 0, end_column: 3, new_text: "x".into() },
            ],
            changes: HashMap::new(),
        };
        assert_eq!(we.affected_file_count(), 2);
        assert_eq!(we.edit_count(), 3);
    }

    #[test]
    fn rename_location_to_edit() {
        let loc = RenameLocation {
            uri: "f.rs".into(),
            line: 3,
            start_column: 4,
            end_column: 8,
            old_text: "foo".into(),
        };
        let edit = loc.to_edit("bar");
        assert_eq!(edit.new_text, "bar");
        assert_eq!(edit.uri, "f.rs");
    }

    #[test]
    fn validate_new_name_ok() {
        assert!(RenameService::validate_new_name("valid_name").is_ok());
    }

    #[test]
    fn validate_new_name_empty() {
        assert_eq!(RenameService::validate_new_name(""), Err(RenameError::EmptyName));
    }

    #[test]
    fn validate_new_name_whitespace() {
        assert_eq!(RenameService::validate_new_name("   "), Err(RenameError::WhitespaceOnly));
    }

    #[test]
    fn validate_new_name_newline() {
        assert_eq!(RenameService::validate_new_name("a\nb"), Err(RenameError::ContainsNewline));
    }

    #[test]
    fn rename_widget_new() {
        let w = RenameInputWidget::new(10, 5, "oldName");
        assert_eq!(w.old_name, "oldName");
        assert_eq!(w.new_name, "oldName");
        assert!(!w.has_changed());
        assert!(w.is_valid);
    }

    #[test]
    fn rename_widget_accept() {
        let mut w = RenameInputWidget::new(1, 1, "old");
        assert!(w.accept().is_none()); // hasn't changed
        w.set_new_name("new");
        assert!(w.has_changed());
        assert_eq!(w.accept(), Some("new"));
    }

    #[test]
    fn rename_widget_invalid_rejects() {
        let mut w = RenameInputWidget::new(1, 1, "old");
        w.set_new_name("");
        assert!(!w.is_valid);
        assert!(w.accept().is_none());
    }

    #[test]
    fn rename_service_no_provider() {
        let svc = RenameService::new();
        let result = svc.compute_edits("f", 1, 1, "new_name");
        assert_eq!(result, Err(RenameError::NoProvider));
    }

    #[test]
    fn rename_edit_span_length() {
        let edit = RenameEdit {
            uri: "f.rs".into(),
            line: 0,
            start_column: 5,
            end_column: 10,
            new_text: "x".into(),
        };
        assert_eq!(edit.span_length(), 5);
    }

    #[test]
    fn rename_edit_char_delta() {
        let edit = RenameEdit {
            uri: "f.rs".into(),
            line: 0,
            start_column: 0,
            end_column: 3,
            new_text: "longer_name".into(),
        };
        assert_eq!(edit.char_delta(), 8);
    }

    #[test]
    fn rename_edit_is_in_file() {
        let edit = RenameEdit {
            uri: "src/main.rs".into(),
            line: 1,
            start_column: 0,
            end_column: 3,
            new_text: "x".into(),
        };
        assert!(edit.is_in_file("src/main.rs"));
        assert!(!edit.is_in_file("src/lib.rs"));
    }

    #[test]
    fn rename_edit_display() {
        let edit = RenameEdit {
            uri: "f.rs".into(),
            line: 5,
            start_column: 2,
            end_column: 8,
            new_text: "bar".into(),
        };
        let s = format!("{}", edit);
        assert!(s.contains("f.rs"));
        assert!(s.contains("bar"));
    }

    #[test]
    fn rename_location_span_and_line() {
        let loc = RenameLocation {
            uri: "x.rs".into(),
            line: 7,
            start_column: 0,
            end_column: 4,
            old_text: "test".into(),
        };
        assert_eq!(loc.span_length(), 4);
        assert!(loc.is_on_line(7));
        assert!(!loc.is_on_line(8));
    }

    #[test]
    fn rename_location_display() {
        let loc = RenameLocation {
            uri: "a.rs".into(),
            line: 1,
            start_column: 0,
            end_column: 3,
            old_text: "foo".into(),
        };
        let s = format!("{}", loc);
        assert!(s.contains("a.rs"));
        assert!(s.contains("foo"));
    }

    #[test]
    fn workspace_edit_edits_for_file() {
        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 3, new_text: "x".into() },
                RenameEdit { uri: "b.rs".into(), line: 2, start_column: 0, end_column: 3, new_text: "x".into() },
                RenameEdit { uri: "a.rs".into(), line: 5, start_column: 0, end_column: 3, new_text: "x".into() },
            ],
            changes: HashMap::new(),
        };
        assert_eq!(we.edits_for_file("a.rs").len(), 2);
        assert_eq!(we.edits_for_file("c.rs").len(), 0);
    }

    #[test]
    fn workspace_edit_merge_and_sort() {
        let mut we1 = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "b.rs".into(), line: 3, start_column: 0, end_column: 2, new_text: "x".into() },
            ],
            changes: HashMap::new(),
        };
        let we2 = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 2, new_text: "y".into() },
            ],
            changes: HashMap::new(),
        };
        we1.merge(we2);
        assert_eq!(we1.edit_count(), 2);
        we1.sort_edits();
        assert_eq!(we1.edits[0].uri, "a.rs");
    }

    #[test]
    fn workspace_edit_affected_uris() {
        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "z.rs".into(), line: 1, start_column: 0, end_column: 1, new_text: "a".into() },
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 1, new_text: "b".into() },
                RenameEdit { uri: "z.rs".into(), line: 2, start_column: 0, end_column: 1, new_text: "c".into() },
            ],
            changes: HashMap::new(),
        };
        let uris = we.affected_uris();
        assert_eq!(uris, vec!["a.rs", "z.rs"]);
    }

    #[test]
    fn workspace_edit_display() {
        let we = WorkspaceEdit::new();
        let s = format!("{}", we);
        assert!(s.contains("0 edits"));
    }

    #[test]
    fn is_valid_identifier_tests() {
        assert!(is_valid_identifier("foo"));
        assert!(is_valid_identifier("_bar"));
        assert!(is_valid_identifier("a1"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1abc"));
        assert!(!is_valid_identifier("a-b"));
        assert!(!is_valid_identifier("a b"));
    }

    #[test]
    fn classify_rename_kinds() {
        assert_eq!(classify_rename("foo", "Foo"), RenameKind::CaseChange);
        assert_eq!(classify_rename("foo", "fooBar"), RenameKind::Augmented);
        assert_eq!(classify_rename("fooBar", "foo"), RenameKind::Augmented);
        assert_eq!(classify_rename("foo", "bar"), RenameKind::FullRename);
    }

    #[test]
    fn rename_history_push_pop() {
        let mut history = RenameHistory::new(3);
        assert!(history.is_empty());

        for i in 0..3 {
            history.push(RenameHistoryEntry {
                old_name: format!("old{}", i),
                new_name: format!("new{}", i),
                edit: WorkspaceEdit::new(),
            });
        }
        assert_eq!(history.len(), 3);

        // Push beyond max evicts oldest
        history.push(RenameHistoryEntry {
            old_name: "old3".into(),
            new_name: "new3".into(),
            edit: WorkspaceEdit::new(),
        });
        assert_eq!(history.len(), 3);
        assert_eq!(history.last().unwrap().old_name, "old3");

        let popped = history.pop().unwrap();
        assert_eq!(popped.new_name, "new3");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn rename_history_clear() {
        let mut history = RenameHistory::default();
        history.push(RenameHistoryEntry {
            old_name: "a".into(),
            new_name: "b".into(),
            edit: WorkspaceEdit::new(),
        });
        assert!(!history.is_empty());
        history.clear();
        assert!(history.is_empty());
    }

    #[test]
    fn rename_error_display() {
        assert_eq!(format!("{}", RenameError::EmptyName), "New name cannot be empty");
        assert_eq!(format!("{}", RenameError::NoProvider), "No rename provider available");
    }

    #[test]
    fn rename_error_file_not_found() {
        let err = RenameError::FileNotFound("missing.rs".into());
        assert!(format!("{}", err).contains("missing.rs"));
    }

    #[test]
    fn rename_range_from_prepare_result() {
        let pr = PrepareRenameResult {
            line: 5,
            start_column: 2,
            end_column: 8,
            placeholder: "myVar".into(),
        };
        let rr = pr.to_range();
        assert_eq!(rr.line, 5);
        assert_eq!(rr.placeholder, "myVar");
    }

    #[test]
    fn prepare_rename_via_service() {
        struct TestProvider;
        impl RenameProvider for TestProvider {
            fn prepare_rename(&self, _uri: &str, _line: u32, _col: u32) -> Option<PrepareRenameResult> {
                Some(PrepareRenameResult {
                    line: 1,
                    start_column: 0,
                    end_column: 3,
                    placeholder: "foo".into(),
                })
            }
            fn provide_rename_edits(&self, _: &str, _: u32, _: u32, _: &str) -> Option<WorkspaceEdit> { None }
        }
        let mut svc = RenameService::new();
        svc.register(Box::new(TestProvider));
        let rr = svc.prepare_rename("f.rs", 1, 1).unwrap();
        assert_eq!(rr.placeholder, "foo");
    }

    #[test]
    fn execute_rename_via_service() {
        struct TestProvider;
        impl RenameProvider for TestProvider {
            fn prepare_rename(&self, _: &str, _: u32, _: u32) -> Option<PrepareRenameResult> { None }
            fn provide_rename_edits(&self, _uri: &str, _: u32, _: u32, new_name: &str) -> Option<WorkspaceEdit> {
                let mut we = WorkspaceEdit::new();
                we.edits.push(RenameEdit {
                    uri: "f.rs".into(), line: 0, start_column: 0, end_column: 3,
                    new_text: new_name.to_string(),
                });
                Some(we)
            }
        }
        let mut svc = RenameService::new();
        svc.register(Box::new(TestProvider));
        let edit = svc.execute_rename("f.rs", 0, 0, "bar").unwrap();
        assert_eq!(edit.edits[0].new_text, "bar");
    }

    #[test]
    fn workspace_edit_add_change() {
        let mut we = WorkspaceEdit::new();
        we.add_change("a.rs", TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 3,
            new_text: "bar".into(),
        });
        assert_eq!(we.changes["a.rs"].len(), 1);
        assert_eq!(we.edit_count(), 1);
        assert_eq!(we.affected_file_count(), 1);
    }

    #[test]
    fn workspace_edit_from_changes() {
        let mut map = HashMap::new();
        map.insert("x.rs".to_string(), vec![TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 1,
            new_text: "Y".into(),
        }]);
        let we = WorkspaceEdit::from_changes(map);
        assert_eq!(we.affected_file_count(), 1);
        assert_eq!(we.edit_count(), 1);
    }

    #[test]
    fn apply_workspace_edit_flat_edits() {
        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "let foo = 1;\nfoo + 2".to_string());

        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "a.rs".into(), line: 0, start_column: 4, end_column: 7, new_text: "bar".into() },
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 3, new_text: "bar".into() },
            ],
            changes: HashMap::new(),
        };

        let result = apply_workspace_edit(&files, &we).unwrap();
        assert!(result["a.rs"].contains("bar"));
        assert!(!result["a.rs"].contains("foo"));
    }

    #[test]
    fn apply_workspace_edit_changes_map() {
        let mut files = HashMap::new();
        files.insert("b.rs".to_string(), "let old = 1;".to_string());

        let mut we = WorkspaceEdit::new();
        we.add_change("b.rs", TextEdit {
            start_line: 0, start_column: 4, end_line: 0, end_column: 7,
            new_text: "new".into(),
        });

        let result = apply_workspace_edit(&files, &we).unwrap();
        assert!(result["b.rs"].contains("new"));
    }

    #[test]
    fn apply_workspace_edit_file_not_found() {
        let files = HashMap::new();
        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "missing.rs".into(), line: 0, start_column: 0, end_column: 1, new_text: "x".into() },
            ],
            changes: HashMap::new(),
        };
        assert!(apply_workspace_edit(&files, &we).is_err());
    }

    #[test]
    fn rename_preview_from_edit() {
        let mut files = HashMap::new();
        files.insert("c.rs".to_string(), "let foo = 1;\nfoo + 2".to_string());

        let we = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "c.rs".into(), line: 0, start_column: 4, end_column: 7, new_text: "bar".into() },
            ],
            changes: HashMap::new(),
        };

        let preview = RenamePreview::from_edit(&files, &we);
        assert_eq!(preview.file_count(), 1);
        assert_eq!(preview.total_changes(), 1);
        let changes = &preview.file_changes["c.rs"];
        assert!(changes[0].1.contains("foo"));
        assert!(changes[0].2.contains("bar"));
    }

    #[test]
    fn workspace_edit_merge_changes() {
        let mut we1 = WorkspaceEdit::new();
        we1.add_change("a.rs", TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 1,
            new_text: "X".into(),
        });
        let mut we2 = WorkspaceEdit::new();
        we2.add_change("a.rs", TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 1,
            new_text: "Y".into(),
        });
        we1.merge(we2);
        assert_eq!(we1.changes["a.rs"].len(), 2);
    }

    #[test]
    fn text_edit_struct() {
        let te = TextEdit {
            start_line: 1, start_column: 2, end_line: 1, end_column: 5,
            new_text: "abc".into(),
        };
        assert_eq!(te.start_line, 1);
        assert_eq!(te.new_text, "abc");
    }

    #[test]
    fn test_rename_location_creation() {
        let loc = ImpactLocation {
            file_path: "src/main.rs".into(),
            line: 10,
            column: 4,
            length: 5,
            context: "let myVar = 1;".into(),
        };
        assert_eq!(loc.file_path, "src/main.rs");
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 4);
        assert_eq!(loc.length, 5);
        assert_eq!(loc.context, "let myVar = 1;");
    }

    #[test]
    fn test_impact_analysis_from_workspace_edit() {
        let mut we = WorkspaceEdit::new();
        we.add_change("a.rs", TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 3,
            new_text: "bar".into(),
        });
        we.add_change("b.rs", TextEdit {
            start_line: 5, start_column: 2, end_line: 5, end_column: 5,
            new_text: "bar".into(),
        });

        let analysis = RenameImpactAnalysis::from_workspace_edit(&we);
        assert_eq!(analysis.files_affected, 2);
        assert_eq!(analysis.total_edits, 2);
        assert_eq!(analysis.locations.len(), 2);
    }

    #[test]
    fn test_impact_analysis_files_list() {
        let mut we = WorkspaceEdit::new();
        we.add_change("a.rs", TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 3,
            new_text: "x".into(),
        });
        we.add_change("b.rs", TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 3,
            new_text: "x".into(),
        });
        we.add_change("a.rs", TextEdit {
            start_line: 2, start_column: 0, end_line: 2, end_column: 3,
            new_text: "x".into(),
        });

        let analysis = RenameImpactAnalysis::from_workspace_edit(&we);
        let files = analysis.files_list();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"a.rs"));
        assert!(files.contains(&"b.rs"));
        assert_eq!(analysis.edits_in_file("a.rs"), 2);
        assert_eq!(analysis.edits_in_file("b.rs"), 1);
    }

    #[test]
    fn test_rename_validate_success() {
        assert!(rename_validate("old", "new").is_ok());
    }

    #[test]
    fn test_rename_validate_failures() {
        // empty
        let err = rename_validate("foo", "").unwrap_err();
        assert!(err.iter().any(|e| e.contains("empty")));

        // same name
        let err = rename_validate("foo", "foo").unwrap_err();
        assert!(err.iter().any(|e| e.contains("differ")));

        // path separator
        let err = rename_validate("foo", "a/b").unwrap_err();
        assert!(err.iter().any(|e| e.contains("path separator")));

        // leading whitespace
        let err = rename_validate("foo", " bar").unwrap_err();
        assert!(err.iter().any(|e| e.contains("whitespace")));
    }

    #[test]
    fn test_rename_validate_multiple_errors() {
        // empty string triggers both "empty" and "same as old" when old is also empty
        // Use a case that triggers exactly two independent errors.
        let err = rename_validate("a/b", "a/b").unwrap_err();
        assert!(err.len() >= 2);
        assert!(err.iter().any(|e| e.contains("differ")));
        assert!(err.iter().any(|e| e.contains("path separator")));
    }

    // ---- RenameConflictDetector tests ----

    #[test]
    fn conflict_detector_finds_conflicts() {
        let mut symbols = HashMap::new();
        symbols.insert("file.rs".to_string(), vec![
            (1, "foo".to_string()),
            (5, "bar".to_string()),
        ]);
        let conflicts = RenameConflictDetector::detect("foo", &symbols);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].line, 1);
    }

    #[test]
    fn conflict_detector_no_conflicts() {
        let mut symbols = HashMap::new();
        symbols.insert("file.rs".to_string(), vec![
            (1, "foo".to_string()),
        ]);
        assert!(!RenameConflictDetector::has_conflict("baz", &symbols));
    }

    #[test]
    fn conflict_detector_has_conflict() {
        let mut symbols = HashMap::new();
        symbols.insert("file.rs".to_string(), vec![
            (1, "foo".to_string()),
        ]);
        assert!(RenameConflictDetector::has_conflict("foo", &symbols));
    }

    // ---- RenameSuggestions tests ----

    #[test]
    fn suggestions_from_camel_case() {
        let suggestions = RenameSuggestions::suggest("myVariable");
        assert!(suggestions.contains(&"my_variable".to_string()));
        assert!(suggestions.contains(&"MYVARIABLE".to_string()));
        assert!(suggestions.contains(&"_myVariable".to_string()));
    }

    #[test]
    fn suggestions_from_snake_case() {
        let suggestions = RenameSuggestions::suggest("my_var");
        assert!(suggestions.contains(&"myVar".to_string()));
        assert!(suggestions.contains(&"MY_VAR".to_string()));
    }

    // ---- RenameScope tests ----

    #[test]
    fn scope_workspace_includes_all() {
        let scope = RenameScope::Workspace;
        assert!(scope.includes_file("any_file.rs"));
    }

    #[test]
    fn scope_single_file() {
        let scope = RenameScope::SingleFile("a.rs".to_string());
        assert!(scope.includes_file("a.rs"));
        assert!(!scope.includes_file("b.rs"));
    }

    #[test]
    fn scope_filter_edit() {
        let mut edit = WorkspaceEdit::new();
        edit.edits.push(RenameEdit {
            uri: "a.rs".into(), line: 1, start_column: 0, end_column: 3, new_text: "bar".into(),
        });
        edit.edits.push(RenameEdit {
            uri: "b.rs".into(), line: 2, start_column: 0, end_column: 3, new_text: "bar".into(),
        });
        let scope = RenameScope::SingleFile("a.rs".to_string());
        let filtered = scope.filter_edit(&edit);
        assert_eq!(filtered.edits.len(), 1);
        assert_eq!(filtered.edits[0].uri, "a.rs");
    }

    #[test]
    fn scope_range_filter() {
        let mut edit = WorkspaceEdit::new();
        edit.edits.push(RenameEdit {
            uri: "a.rs".into(), line: 5, start_column: 0, end_column: 3, new_text: "x".into(),
        });
        edit.edits.push(RenameEdit {
            uri: "a.rs".into(), line: 15, start_column: 0, end_column: 3, new_text: "x".into(),
        });
        let scope = RenameScope::Range { file: "a.rs".to_string(), start_line: 0, end_line: 10 };
        let filtered = scope.filter_edit(&edit);
        assert_eq!(filtered.edits.len(), 1);
        assert_eq!(filtered.edits[0].line, 5);
    }

    // ---- RenameHistory extensions ----

    #[test]
    fn history_summary() {
        let mut history = RenameHistory::new(10);
        history.push(RenameHistoryEntry {
            old_name: "foo".into(),
            new_name: "bar".into(),
            edit: WorkspaceEdit::new(),
        });
        let summary = history.summary();
        assert_eq!(summary, vec!["foo → bar"]);
    }

    #[test]
    fn history_entries_for_file() {
        let mut history = RenameHistory::new(10);
        let mut edit = WorkspaceEdit::new();
        edit.edits.push(RenameEdit {
            uri: "main.rs".into(), line: 1, start_column: 0, end_column: 3, new_text: "bar".into(),
        });
        history.push(RenameHistoryEntry {
            old_name: "foo".into(),
            new_name: "bar".into(),
            edit,
        });
        assert_eq!(history.entries_for_file("main.rs").len(), 1);
        assert_eq!(history.entries_for_file("other.rs").len(), 0);
    }

    // -- RenameRefactorPlan --

    #[test]
    fn refactor_plan_ordered_steps() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("c", "c2", RenameScope::Workspace, 3);
        plan.add_step("a", "a2", RenameScope::Workspace, 1);
        plan.add_step("b", "b2", RenameScope::Workspace, 2);
        let ordered = plan.ordered_steps();
        assert_eq!(ordered[0].old_name, "a");
        assert_eq!(ordered[1].old_name, "b");
        assert_eq!(ordered[2].old_name, "c");
    }

    #[test]
    fn refactor_plan_validate() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("foo", "bar", RenameScope::Workspace, 1);
        plan.add_step("baz", "", RenameScope::Workspace, 2); // invalid: empty new name
        let issues = plan.validate();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].0, 1); // index of invalid step
    }

    #[test]
    fn refactor_plan_chained_steps() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("a", "b", RenameScope::Workspace, 1);
        plan.add_step("b", "c", RenameScope::Workspace, 2);
        let chains = plan.chained_steps();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0], (0, 1));
    }

    #[test]
    fn refactor_plan_affected_files() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("x", "y", RenameScope::SingleFile("main.rs".into()), 1);
        plan.add_step("a", "b", RenameScope::SingleFile("lib.rs".into()), 2);
        plan.add_step("c", "d", RenameScope::Workspace, 3);
        let files = plan.affected_files();
        assert_eq!(files, vec!["lib.rs", "main.rs"]);
    }

    #[test]
    fn refactor_plan_remove_step() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("a", "b", RenameScope::Workspace, 1);
        plan.add_step("c", "d", RenameScope::Workspace, 2);
        assert_eq!(plan.len(), 2);
        let removed = plan.remove_step(0).unwrap();
        assert_eq!(removed.old_name, "a");
        assert_eq!(plan.len(), 1);
        assert!(plan.remove_step(5).is_none());
    }

    #[test]
    fn refactor_plan_display() {
        let mut plan = RenameRefactorPlan::new();
        plan.add_step("foo", "bar", RenameScope::Workspace, 1);
        let display = format!("{}", plan);
        assert!(display.contains("1 steps"));
        assert!(display.contains("foo → bar"));
    }

    #[test]
    fn rename_step_display() {
        let step = RenameStep {
            old_name: "old".into(),
            new_name: "new".into(),
            scope: RenameScope::Workspace,
            priority: 5,
        };
        assert_eq!(format!("{}", step), "old → new (priority 5)");
    }

    // ---- Case transformation tests ----

    #[test]
    fn test_to_snake_case_from_camel() {
        assert_eq!(to_snake_case("myVariableName"), "my_variable_name");
        assert_eq!(to_snake_case("HTMLParser"), "html_parser");
        assert_eq!(to_snake_case("simpleXMLReader"), "simple_xml_reader");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("A"), "a");
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn test_to_camel_case_from_snake() {
        assert_eq!(to_camel_case("my_variable_name"), "myVariableName");
        assert_eq!(to_camel_case("get_http_response"), "getHttpResponse");
        assert_eq!(to_camel_case("already"), "already");
        assert_eq!(to_camel_case("PascalCase"), "pascalCase");
        assert_eq!(to_camel_case("kebab-case"), "kebabCase");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("my_struct"), "MyStruct");
        assert_eq!(to_pascal_case("some_type_name"), "SomeTypeName");
        assert_eq!(to_pascal_case("already"), "Already");
        assert_eq!(to_pascal_case("kebab-case"), "KebabCase");
    }

    #[test]
    fn test_to_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("myConstant"), "MY_CONSTANT");
        assert_eq!(to_screaming_snake_case("max_size"), "MAX_SIZE");
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("myComponent"), "my-component");
        assert_eq!(to_kebab_case("some_name"), "some-name");
    }

    // ---- BatchRename tests ----

    #[test]
    fn test_batch_rename_compute_edits() {
        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "let foo = foo + 1;\nfoo".to_string());
        files.insert("b.rs".to_string(), "fn bar() {}".to_string());

        let batch = BatchRename::new("foo", "baz");
        let we = batch.compute_edits(&files);

        // "foo" appears 3 times in a.rs, 0 in b.rs
        let a_edits: Vec<_> = we.edits.iter().filter(|e| e.uri == "a.rs").collect();
        assert_eq!(a_edits.len(), 3);
        assert!(we.edits.iter().all(|e| e.new_text == "baz"));
    }

    #[test]
    fn test_batch_rename_count_occurrences() {
        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "foo foo foo".to_string());
        files.insert("b.rs".to_string(), "bar".to_string());

        let batch = BatchRename::new("foo", "baz");
        assert_eq!(batch.count_occurrences(&files), 3);
    }

    #[test]
    fn test_batch_rename_no_matches() {
        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "let x = 1;".to_string());

        let batch = BatchRename::new("nonexistent", "replacement");
        let we = batch.compute_edits(&files);
        assert!(we.edits.is_empty());
        assert_eq!(batch.count_occurrences(&files), 0);
    }

    // ---- RenameUndoInfo tests ----

    #[test]
    fn test_undo_info_prepare_and_restore() {
        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "let foo = 1;".to_string());
        files.insert("b.rs".to_string(), "let bar = 2;".to_string());
        files.insert("c.rs".to_string(), "unrelated".to_string());

        let mut we = WorkspaceEdit::new();
        we.edits.push(RenameEdit {
            uri: "a.rs".into(), line: 0, start_column: 4, end_column: 7,
            new_text: "baz".into(),
        });

        let undo = RenameUndoInfo::prepare(&files, &we, "foo", "baz");
        assert_eq!(undo.file_count(), 1);
        assert_eq!(undo.old_name, "foo");
        assert_eq!(undo.new_name, "baz");

        // Simulate applying the edit then restoring
        let mut modified = files.clone();
        modified.insert("a.rs".to_string(), "let baz = 1;".to_string());
        undo.restore_into(&mut modified);
        assert_eq!(modified["a.rs"], "let foo = 1;");
    }

    // ---- RenameProgress tests ----

    #[test]
    fn test_rename_progress_tracking() {
        let mut progress = RenameProgress::new(vec![
            "a.rs".to_string(),
            "b.rs".to_string(),
            "c.rs".to_string(),
        ]);

        assert!(!progress.is_done());
        assert!(!progress.has_failures());
        assert!((progress.progress_fraction() - 0.0).abs() < f64::EPSILON);

        progress.mark_completed("a.rs");
        assert!(!progress.is_done());
        assert!((progress.progress_fraction() - 1.0 / 3.0).abs() < 0.01);

        progress.mark_failed("b.rs", "permission denied");
        assert!(progress.has_failures());

        progress.mark_completed("c.rs");
        assert!(progress.is_done());
        assert!((progress.progress_fraction() - 1.0).abs() < f64::EPSILON);
        assert_eq!(progress.completed.len(), 2);
        assert_eq!(progress.failed.len(), 1);
        assert_eq!(progress.failed[0].1, "permission denied");
    }

    #[test]
    fn test_rename_progress_empty() {
        let progress = RenameProgress::new(vec![]);
        assert!(progress.is_done());
        assert!((progress.progress_fraction() - 1.0).abs() < f64::EPSILON);
    }

    // ---- validate_rename_detailed tests ----

    #[test]
    fn test_validate_rename_detailed_valid() {
        let result = validate_rename_detailed("foo", "bar");
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_rename_detailed_errors() {
        let result = validate_rename_detailed("foo", "");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("empty")));

        let result = validate_rename_detailed("foo", "foo");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("identical")));

        let result = validate_rename_detailed("foo", "not-valid");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("valid identifier")));
    }

    #[test]
    fn test_validate_rename_detailed_warnings() {
        let result = validate_rename_detailed("foo", "_bar");
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("underscore")));

        let result = validate_rename_detailed("foo", "x");
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("single-character")));

        let long_name = "a".repeat(65);
        let result = validate_rename_detailed("foo", &long_name);
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("long")));
    }

    #[test]
    fn test_validate_rename_mixed_convention_warning() {
        let result = validate_rename_detailed("foo", "my_camelCase");
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("mixes")));

        // SCREAMING_SNAKE should NOT trigger mixed warning
        let result = validate_rename_detailed("foo", "MAX_SIZE");
        assert!(result.is_valid);
        assert!(!result.warnings.iter().any(|w| w.contains("mixes")));
    }

    // ---- Reserved name / keyword tests ----

    #[test]
    fn test_is_reserved_name() {
        assert!(is_reserved_name("fn", RUST_KEYWORDS));
        assert!(is_reserved_name("let", RUST_KEYWORDS));
        assert!(is_reserved_name("struct", RUST_KEYWORDS));
        assert!(!is_reserved_name("my_func", RUST_KEYWORDS));
        assert!(!is_reserved_name("FN", RUST_KEYWORDS)); // case-sensitive
    }

    #[test]
    fn preview_gen_add_and_count() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 1, 5, "old", "new"));
        assert_eq!(preview_gen.change_count(), 1);
        assert_eq!(preview_gen.affected_file_count(), 1);
    }

    #[test]
    fn preview_gen_multiple_files() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 1, 5, "x", "y"));
        preview_gen.add_change(RenamePreviewChange::new("b.rs", 10, 3, "x", "y"));
        assert_eq!(preview_gen.affected_file_count(), 2);
    }

    #[test]
    fn preview_gen_changes_by_file() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 1, 0, "x", "y"));
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 5, 0, "x", "y"));
        preview_gen.add_change(RenamePreviewChange::new("b.rs", 2, 0, "x", "y"));
        let by_file = preview_gen.changes_by_file();
        assert_eq!(by_file["a.rs"].len(), 2);
        assert_eq!(by_file["b.rs"].len(), 1);
    }

    #[test]
    fn preview_gen_generate_preview() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("main.rs", 10, 4, "foo", "bar"));
        let preview = preview_gen.generate_preview();
        assert!(preview.contains("main.rs"));
        assert!(preview.contains("foo"));
        assert!(preview.contains("bar"));
    }

    #[test]
    fn preview_gen_affected_files_sorted() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("z.rs", 1, 0, "a", "b"));
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 1, 0, "a", "b"));
        let files = preview_gen.affected_files();
        assert_eq!(files, vec!["a.rs", "z.rs"]);
    }

    #[test]
    fn preview_gen_clear_and_display() {
        let mut preview_gen = RenamePreviewGenerator::new();
        preview_gen.add_change(RenamePreviewChange::new("a.rs", 1, 0, "x", "y"));
        assert!(format!("{preview_gen}").contains("1 changes"));
        preview_gen.clear();
        assert_eq!(preview_gen.change_count(), 0);
    }

    #[test]
    fn preview_change_diff_line() {
        let c = RenamePreviewChange::new("lib.rs", 42, 8, "old_name", "new_name");
        let d = c.diff_line();
        assert!(d.contains("lib.rs:42:8"));
        assert!(d.contains("old_name"));
    }

    #[test]
    fn conflict_resolver_keyword_conflict() {
        let mut resolver = RenameConflictResolver::new();
        resolver.add_keyword("fn");
        let conflicts = resolver.check("fn", "main.rs", 10);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, RenameConflictKind::ReservedKeyword);
        assert!(conflicts[0].suggestion.is_some());
    }

    #[test]
    fn conflict_resolver_existing_name() {
        let mut resolver = RenameConflictResolver::new();
        resolver.add_existing_name("my_var");
        let conflicts = resolver.check("my_var", "lib.rs", 5);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, RenameConflictKind::NameAlreadyExists);
    }

    #[test]
    fn conflict_resolver_import_collision() {
        let mut resolver = RenameConflictResolver::new();
        resolver.add_import("HashMap");
        let conflicts = resolver.check("HashMap", "lib.rs", 1);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, RenameConflictKind::ImportCollision);
    }

    #[test]
    fn conflict_resolver_no_conflict() {
        let resolver = RenameConflictResolver::new();
        let conflicts = resolver.check("unique_name", "lib.rs", 1);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflict_resolver_suggest_alternative() {
        let mut resolver = RenameConflictResolver::new();
        resolver.add_existing_name("count");
        let alt = resolver.suggest_alternative("count");
        assert_eq!(alt, "count_1");
    }

    #[test]
    fn conflict_resolver_suggest_no_conflict() {
        let resolver = RenameConflictResolver::new();
        let alt = resolver.suggest_alternative("unique");
        assert_eq!(alt, "unique");
    }

    #[test]
    fn conflict_resolver_display_and_clear() {
        let mut resolver = RenameConflictResolver::new();
        resolver.add_existing_name("x");
        resolver.add_keyword("fn");
        assert!(format!("{resolver}").contains("names=1"));
        resolver.clear();
        assert_eq!(resolver.existing_name_count(), 0);
    }

    #[test]
    fn conflict_kind_display() {
        assert_eq!(format!("{}", RenameConflictKind::NameAlreadyExists), "name already exists");
        assert_eq!(format!("{}", RenameConflictKind::ShadowsOuterScope), "shadows outer scope");
        assert_eq!(format!("{}", RenameConflictKind::ReservedKeyword), "reserved keyword");
        assert_eq!(format!("{}", RenameConflictKind::ImportCollision), "import collision");
    }

    #[test]
    fn detected_conflict_display() {
        let c = DetectedRenameConflict::new("fn", "main.rs", 10, RenameConflictKind::ReservedKeyword)
            .with_suggestion("r#fn");
        let s = format!("{c}");
        assert!(s.contains("fn"));
        assert!(s.contains("main.rs"));
        assert!(s.contains("r#fn"));
    }


    // -- rename additional tests -------------------------------------------

    #[test]
    fn x_rename_panel_state_new() {
        let p = XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XRenameLayoutRegion::Sidebar);
    }

    #[test]
    fn x_rename_panel_area() {
        let p = XRenamePanelState::new(XRenameLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_rename_panel_toggle() {
        let mut p = XRenamePanelState::new(XRenameLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_rename_panel_resize() {
        let mut p = XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_rename_panel_is_narrow() {
        let mut p = XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_rename_total_visible_area_basic() {
        let panels = vec![
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "a"),
            XRenamePanelState::new(XRenameLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_rename_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_rename_total_visible_area_hidden() {
        let mut panels = vec![
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "a"),
            XRenamePanelState::new(XRenameLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_rename_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_rename_count_in_region_basic() {
        let panels = vec![
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "a"),
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "b"),
            XRenamePanelState::new(XRenameLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_rename_count_in_region(&panels, XRenameLayoutRegion::Sidebar), 2);
        assert_eq!(x_rename_count_in_region(&panels, XRenameLayoutRegion::Editor), 1);
        assert_eq!(x_rename_count_in_region(&panels, XRenameLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_rename_widest_panel_basic() {
        let mut panels = vec![
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "narrow"),
            XRenamePanelState::new(XRenameLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_rename_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_rename_collapse_region_basic() {
        let mut panels = vec![
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "a"),
            XRenamePanelState::new(XRenameLayoutRegion::Sidebar, "b"),
            XRenamePanelState::new(XRenameLayoutRegion::Editor, "c"),
        ];
        x_rename_collapse_region(&mut panels, XRenameLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_rename_layout_constraint_clamp() {
        let lc = XRenameLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_rename_layout_constraint_satisfied() {
        let lc = XRenameLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_rename_widest_panel_empty() {
        let panels: Vec<XRenamePanelState> = vec![];
        assert!(x_rename_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_rename_layout_region_eq() {
        assert_eq!(XRenameLayoutRegion::Sidebar, XRenameLayoutRegion::Sidebar);
        assert_ne!(XRenameLayoutRegion::Sidebar, XRenameLayoutRegion::Panel);
    }


    // -- rename Z-extended tests -----------------------------------------------

    #[test]
    fn z_rename_priority_weight() {
        assert_eq!(ZRenamePriority::Idle.weight(), 0);
        assert_eq!(ZRenamePriority::Normal.weight(), 2);
        assert_eq!(ZRenamePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_rename_priority_label() {
        assert_eq!(ZRenamePriority::Low.label(), "low");
        assert_eq!(ZRenamePriority::High.label(), "high");
    }

    #[test]
    fn z_rename_priority_is_elevated() {
        assert!(!ZRenamePriority::Normal.is_elevated());
        assert!(ZRenamePriority::High.is_elevated());
        assert!(ZRenamePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_rename_priority_display() {
        assert_eq!(format!("{}", ZRenamePriority::Idle), "idle");
    }

    #[test]
    fn z_rename_priority_all_asc() {
        let all = ZRenamePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZRenamePriority::Idle);
        assert_eq!(all[4], ZRenamePriority::Realtime);
    }

    #[test]
    fn z_rename_struct_new() {
        let s = ZRenameRenameAuditTrail::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_rename_struct_toggled_clone() {
        let s = ZRenameRenameAuditTrail::new();
        let t = s.toggled_clone();
        assert_ne!(s.approved, t.approved);
    }

    #[test]
    fn z_rename_rolling_hash_deterministic() {
        let h1 = z_rename_rolling_hash(b"test");
        let h2 = z_rename_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_rename_rolling_hash(b"a"), z_rename_rolling_hash(b"b"));
    }

    #[test]
    fn z_rename_pad_to_basic() {
        assert_eq!(z_rename_pad_to("hi", 5), "hi   ");
        assert_eq!(z_rename_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_rename_is_identifier_basic() {
        assert!(z_rename_is_identifier("foo_bar"));
        assert!(z_rename_is_identifier("abc123"));
        assert!(!z_rename_is_identifier(""));
        assert!(!z_rename_is_identifier("has space"));
    }

    #[test]
    fn z_rename_levenshtein_basic() {
        assert_eq!(z_rename_levenshtein("", ""), 0);
        assert_eq!(z_rename_levenshtein("abc", "abc"), 0);
        assert_eq!(z_rename_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_rename_unique_words_basic() {
        let w = z_rename_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_rename_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_rename_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_rename_common_prefix_basic() {
        assert_eq!(z_rename_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_rename_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_rename_struct_clear() {
        let mut s = ZRenameRenameAuditTrail::new();
        s.records.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_rename_rolling_hash_empty() {
        let h = z_rename_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
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
    fn xb_ring_buffer_36_push_and_len() {
        let mut rb = super::XbRingBuffer36::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_36_overwrite() {
        let mut rb = super::XbRingBuffer36::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_36_get_out_of_bounds() {
        let rb = super::XbRingBuffer36::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_36_drain_all() {
        let mut rb = super::XbRingBuffer36::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_36_peek_front_back() {
        let mut rb = super::XbRingBuffer36::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_36_clear() {
        let mut rb = super::XbRingBuffer36::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_36_capacity() {
        let rb = super::XbRingBuffer36::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_36_basic() {
        let h = super::xb_fnv1a_36(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_36(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_36_different_inputs() {
        let h1 = super::xb_fnv1a_36(b"abc");
        let h2 = super::xb_fnv1a_36(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_36_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_36(&data);
        let dec = super::xb_rle_decode_36(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_36_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_36(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_36(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_36_values() {
        assert!((super::xb_clamp_36(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_36(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_36(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_36_values() {
        assert!((super::xb_lerp_36(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_36(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_36(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_36_wrap_around_twice() {
        let mut rb = super::XbRingBuffer36::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }

}
