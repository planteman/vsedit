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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 150
// ---------------------------------------------------------------------------

/// Generic object pool `Xc150Pool<T>`.
pub struct Xc150Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc150Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc150PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc150Pool<T> {
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
    pub fn stats(&self) -> Xc150PoolStats {
        Xc150PoolStats {
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

impl<T> Default for Xc150Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc150Scheduler`.
pub struct Xc150Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc150Scheduler {
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

impl Default for Xc150Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_150 hash for the given byte slice.
pub fn xc_150_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_150 convention.
pub fn xc_150_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe49 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe49Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe49PipelineError {
    pub stage: Xe49Stage,
    pub message: String,
}

impl std::fmt::Display for Xe49PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe49Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe49Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError>>>,
    stage_names: Vec<Xe49Stage>,
}

impl Xe49Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe49Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe49Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe49Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe49Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
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

    pub fn compose(mut self, other: Xe49Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe49CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe49CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe49Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe49CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe49CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe49Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe49CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_49_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe49CacheEntry {
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

    fn xe_49_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe49CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_49_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
    Ok(data)
}

pub fn xe_49_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_49_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_49_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_49_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe49PipelineError> {
    Err(Xe49PipelineError {
        stage: Xe49Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_38: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg38Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg38Graph {
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

impl Default for Xg38Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_38: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg38Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg38Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg38Heap<T>) {
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

impl<T: Ord> Default for Xg38Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 149).
pub struct Xh149SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh149SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 191 as u64,
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

/// A compact bit set supporting boolean operations (variant 149).
pub struct Xh149BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh149BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 149).
pub struct Xi149Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi149Deque<T> {
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
pub struct Xi149Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi149Interval {
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

/// A simple interval tree (variant 149).
pub struct Xi149IntervalTree {
    xi_intervals: Vec<Xi149Interval>,
}

impl Xi149IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi149Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi149Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi149Interval) -> Vec<&Xi149Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi149Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi149Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi149Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi149Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi149Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi149Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 149) ---

/// Disjoint set / union-find for crate 149.
pub struct Xj149UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj149UnionFind {
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

const XJ149_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 149.
pub struct Xj149BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj149BTreeNode<K, V>>>,
    len: usize,
}

struct Xj149BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj149BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj149BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ149_BTREE_ORDER - 1
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
        let mid = XJ149_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj149BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj149BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj149BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj149BTreeNode::xj_new_leaf();
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


// --- xk_149 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk149SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk149SegmentTree {
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
pub struct Xk149DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk149DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_149).
#[derive(Debug, Clone)]
pub struct Xl149Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl149Rope {
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

/// Suffix array for efficient string searching (xl_149).
#[derive(Debug, Clone)]
pub struct Xl149SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl149SuffixArray {
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
pub struct Xm149MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm149MatrixSparse {
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
pub struct Xm149Tokenizer {
    text: String,
}

impl Xm149Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 149.
pub struct Xn149Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn149Fenwick {
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

// ----- AVL tree map — crate 149 -----

#[derive(Debug, Clone)]
struct Xn149AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn149AvlNode<K, V>>>,
    right: Option<Box<Xn149AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 149.
#[derive(Debug, Clone)]
pub struct Xn149AVL<K, V> {
    root: Option<Box<Xn149AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn149AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn149AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn149AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn149AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn149AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn149AvlNode<K, V>>) -> Box<Xn149AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn149AvlNode<K, V>>) -> Box<Xn149AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn149AvlNode<K, V>>) -> Box<Xn149AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn149AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn149AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn149AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn149AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn149AvlNode<K, V>>) -> &Xn149AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn149AvlNode<K, V>>) -> (Box<Xn149AvlNode<K, V>>, Option<Box<Xn149AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn149AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn149AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn149AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn149AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn149AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn149AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn149AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo149RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo149Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo149RBNode<K, V> {
    key: K,
    value: V,
    color: Xo149Color,
    left: Option<Box<Xo149RBNode<K, V>>>,
    right: Option<Box<Xo149RBNode<K, V>>>,
}

/// A red-black tree map for crate 149.
#[derive(Debug, Clone)]
pub struct Xo149RedBlack<K, V> {
    root: Option<Box<Xo149RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo149RedBlack<K, V> {
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
            r.color = Xo149Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo149RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo149RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo149RBNode {
                    key, value, color: Xo149Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo149RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo149Color::Red)
    }

    fn xo_balance(mut h: Box<Xo149RBNode<K, V>>) -> Box<Xo149RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo149Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo149RBNode<K, V>>) -> Box<Xo149RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo149Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo149RBNode<K, V>>) -> Box<Xo149RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo149Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo149RBNode<K, V>>) {
        h.color = Xo149Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo149Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo149Color::Black; }
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
            r.color = Xo149Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo149RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo149RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo149RBNode<K, V>) -> (K, V, Option<Box<Xo149RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo149RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo149Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo149RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo149ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 149.
#[derive(Debug, Clone)]
pub struct Xo149ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo149ConsistentHash {
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
            let vkey = format!("{}#xo149#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo149#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 149).
#[derive(Debug)]
pub struct Xp149SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp149Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp149Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp149Node<K, V>>>,
    xp_right: Option<Box<Xp149Node<K, V>>>,
}

impl<K: Ord, V> Xp149Node<K, V> {
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

impl<K: Ord, V> Default for Xp149SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp149SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp149Node<K, V>>>, key: &K) -> Option<Box<Xp149Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp149Node<K, V>>) -> Box<Xp149Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp149Node<K, V>>) -> Box<Xp149Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp149Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp149Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp149Node::xp_new(key, val));
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


    // ---- xc_ pool / scheduler tests – block 150 ----

    #[test]
    fn xc_150_pool_new_empty() {
        let pool: super::Xc150Pool<i32> = super::Xc150Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_150_pool_release_acquire() {
        let mut pool = super::Xc150Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_150_pool_acquire_empty() {
        let mut pool: super::Xc150Pool<i32> = super::Xc150Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_150_pool_full() {
        let mut pool = super::Xc150Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_150_pool_drain() {
        let mut pool = super::Xc150Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_150_pool_stats() {
        let mut pool = super::Xc150Pool::new(8);
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
    fn xc_150_pool_clear() {
        let mut pool = super::Xc150Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_150_pool_shrink() {
        let mut pool = super::Xc150Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_150_pool_default() {
        let pool: super::Xc150Pool<String> = super::Xc150Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_150_pool_extend() {
        let mut pool = super::Xc150Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_150_pool_retain() {
        let mut pool = super::Xc150Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_150_scheduler_round_robin() {
        let mut sched = super::Xc150Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_150_scheduler_empty() {
        let mut sched = super::Xc150Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_150_scheduler_reset() {
        let mut sched = super::Xc150Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_150_scheduler_add_remove() {
        let mut sched = super::Xc150Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_150_scheduler_targets() {
        let sched = super::Xc150Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_150_hash_empty() {
        assert_eq!(super::xc_150_hash(b""), 5381);
    }

    #[test]
    fn xc_150_hash_data() {
        let h = super::xc_150_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_150_hash(b"hello"), h);
    }

    #[test]
    fn xc_150_reverse_str() {
        assert_eq!(super::xc_150_reverse("abc"), "cba");
        assert_eq!(super::xc_150_reverse(""), "");
    }


    #[test]
    fn xe_49_pipeline_empty() {
        let p = super::Xe49Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_49_pipeline_parse_stage() {
        let p = super::Xe49Pipeline::new()
            .add_parse(super::xe_49_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_49_pipeline_transform_double() {
        let p = super::Xe49Pipeline::new()
            .add_transform(super::xe_49_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_49_pipeline_validate_reverse() {
        let p = super::Xe49Pipeline::new()
            .add_validate(super::xe_49_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_49_pipeline_emit_filter() {
        let p = super::Xe49Pipeline::new()
            .add_emit(super::xe_49_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_49_pipeline_multi_stage() {
        let p = super::Xe49Pipeline::new()
            .add_parse(super::xe_49_pipeline_identity)
            .add_transform(super::xe_49_pipeline_double)
            .add_validate(super::xe_49_pipeline_reverse)
            .add_emit(super::xe_49_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_49_pipeline_error_propagation() {
        let p = super::Xe49Pipeline::new()
            .add_parse(super::xe_49_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe49Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_49_pipeline_compose() {
        let p1 = super::Xe49Pipeline::new()
            .add_parse(super::xe_49_pipeline_identity);
        let p2 = super::Xe49Pipeline::new()
            .add_transform(super::xe_49_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_49_pipeline_error_display() {
        let e = super::Xe49PipelineError {
            stage: super::Xe49Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_49_cache_put_get() {
        let mut c = super::Xe49Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_49_cache_miss() {
        let mut c: super::Xe49Cache<&str, i32> = super::Xe49Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_49_cache_ttl_expiry() {
        let mut c = super::Xe49Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_49_cache_evict() {
        let mut c = super::Xe49Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_49_cache_capacity() {
        let mut c = super::Xe49Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_49_cache_stats() {
        let mut c = super::Xe49Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_49_cache_clear() {
        let mut c = super::Xe49Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_38 graph tests ------------------------------------------------

    #[test]
    fn xg_38_graph_empty() {
        let g = super::Xg38Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_38_graph_add_node() {
        let mut g = super::Xg38Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_38_graph_add_edge() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_38_graph_neighbors() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_38_graph_has_path() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_38_graph_self_path() {
        let g = super::Xg38Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_38_graph_topo_sort() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_38_graph_cycle_detect_false() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_38_graph_cycle_detect_true() {
        let mut g = super::Xg38Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_38 heap tests -------------------------------------------------

    #[test]
    fn xg_38_heap_empty() {
        let h: super::Xg38Heap<i32> = super::Xg38Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_38_heap_push_pop() {
        let mut h = super::Xg38Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_38_heap_peek() {
        let mut h = super::Xg38Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_38_heap_drain_sorted() {
        let mut h = super::Xg38Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_38_heap_merge() {
        let mut a = super::Xg38Heap::new();
        let mut b = super::Xg38Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_38_heap_default() {
        let h: super::Xg38Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_38_graph_default() {
        let g: super::Xg38Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh149_skip_insert_contains() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh149_skip_remove() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh149_skip_len() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh149_skip_range_query() {
        let mut sl = super::Xh149SkipList::xh_new(4);
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
    fn xh149_skip_floor_ceiling() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh149_skip_rank() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh149_skip_empty() {
        let sl = super::Xh149SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh149_skip_duplicates() {
        let mut sl = super::Xh149SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh149_bitset_set_test() {
        let mut bs = super::Xh149BitSet::xh_new(256);
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
    fn xh149_bitset_clear_count() {
        let mut bs = super::Xh149BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh149_bitset_and_or_xor() {
        let mut a = super::Xh149BitSet::xh_new(128);
        let mut b = super::Xh149BitSet::xh_new(128);
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
    fn xh149_bitset_iter_ones() {
        let mut bs = super::Xh149BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh149_bitset_first_last() {
        let mut bs = super::Xh149BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh149_bitset_empty() {
        let bs = super::Xh149BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi149_deque_push_pop_back() {
        let mut dq = super::Xi149Deque::xi_new(4);
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
    fn xi149_deque_push_pop_front() {
        let mut dq = super::Xi149Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi149_deque_mixed_ops() {
        let mut dq = super::Xi149Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi149_deque_get_and_split() {
        let mut dq = super::Xi149Deque::xi_new(8);
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
    fn xi149_deque_rotate_left() {
        let mut dq = super::Xi149Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi149_deque_rotate_right() {
        let mut dq = super::Xi149Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi149_deque_grow() {
        let mut dq = super::Xi149Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi149_deque_empty() {
        let dq = super::Xi149Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi149_interval_tree_insert_query() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi149Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi149Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi149_interval_tree_overlap() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi149Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi149Interval::xi_new(12, 20));
        let q = super::Xi149Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi149_interval_tree_remove() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi149Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi149_interval_tree_gaps() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi149Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi149Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi149Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi149Interval::xi_new(8, 10));
    }

    #[test]
    fn xi149_interval_tree_merge() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi149Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi149Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi149Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi149Interval::xi_new(10, 15));
    }

    #[test]
    fn xi149_interval_tree_all() {
        let mut tree = super::Xi149IntervalTree::xi_new();
        tree.xi_insert(super::Xi149Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi149Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi149_interval_tree_empty() {
        let tree = super::Xi149IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi149_interval_tree_contains_point() {
        let iv = super::Xi149Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 149) ---

    #[test]
    fn xj_149_uf_make_and_find() {
        let mut uf = super::Xj149UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_149_uf_union_connected() {
        let mut uf = super::Xj149UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_149_uf_component_count() {
        let mut uf = super::Xj149UnionFind::xj_new();
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
    fn xj_149_uf_component_size() {
        let mut uf = super::Xj149UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_149_uf_largest_component() {
        let mut uf = super::Xj149UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_149_uf_many_elements() {
        let mut uf = super::Xj149UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_149_uf_separate_components() {
        let mut uf = super::Xj149UnionFind::xj_new();
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
    fn xj_149_uf_path_compression() {
        let mut uf = super::Xj149UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_149_bt_insert_get() {
        let mut bt = super::Xj149BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_149_bt_contains_len() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_149_bt_replace() {
        let mut bt = super::Xj149BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_149_bt_remove() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_149_bt_keys_values() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_149_bt_range() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_149_bt_min_max() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_149_bt_many_inserts() {
        let mut bt = super::Xj149BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_149 segment tree tests ---

    #[test]
    fn xk_149_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_149_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk149SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_149_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_149_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_149_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_149_st_single_element() {
        let data = vec![42];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_149_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk149SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_149_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk149SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_149 disjoint intervals tests ---

    #[test]
    fn xk_149_di_add_and_count() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_149_di_merge_overlap() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_149_di_contains() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_149_di_remove() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_149_di_covered_length() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_149_di_gaps() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_149_di_merge_adjacent() {
        let mut di = super::Xk149DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_149_di_empty() {
        let di = super::Xk149DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_149_rope_new_empty() {
        let rope = super::Xl149Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_149_rope_from_str() {
        let rope = super::Xl149Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_149_rope_insert_at() {
        let mut rope = super::Xl149Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_149_rope_delete_range() {
        let mut rope = super::Xl149Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_149_rope_char_at() {
        let rope = super::Xl149Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_149_rope_split_concat() {
        let rope = super::Xl149Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_149_rope_line_count() {
        let rope = super::Xl149Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_149_rope_line_at() {
        let rope = super::Xl149Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_149_sa_build_and_search() {
        let sa = super::Xl149SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_149_sa_count() {
        let sa = super::Xl149SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_149_sa_longest_repeated() {
        let sa = super::Xl149SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_149_sa_all_positions() {
        let sa = super::Xl149SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_149_sa_len() {
        let sa = super::Xl149SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_149_sa_empty() {
        let sa = super::Xl149SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_149_rope_slice() {
        let rope = super::Xl149Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_149_sa_search_start() {
        let sa = super::Xl149SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_149_sparse_set_get() {
        let mut m = super::Xm149MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_149_sparse_row_col() {
        let mut m = super::Xm149MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_149_sparse_transpose() {
        let mut m = super::Xm149MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_149_sparse_multiply_vec() {
        let mut m = super::Xm149MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_149_sparse_nnz_density() {
        let mut m = super::Xm149MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_149_sparse_clear() {
        let mut m = super::Xm149MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_149_sparse_overwrite_zero() {
        let mut m = super::Xm149MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_149_tokenizer_basic() {
        let t = super::Xm149Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_149_tokenizer_count() {
        let t = super::Xm149Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_149_tokenizer_unique() {
        let t = super::Xm149Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_149_tokenizer_frequency() {
        let t = super::Xm149Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_149_tokenizer_delimiter() {
        let t = super::Xm149Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_149_tokenizer_whitespace() {
        let t = super::Xm149Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_149_tokenizer_empty() {
        let t = super::Xm149Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 149 ----

    #[test]
    fn xn_149_fenwick_prefix_sum() {
        let mut ft = super::Xn149Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_149_fenwick_range_sum() {
        let mut ft = super::Xn149Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_149_fenwick_point_query() {
        let mut ft = super::Xn149Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_149_fenwick_len() {
        let ft = super::Xn149Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_149_fenwick_multiple_updates() {
        let mut ft = super::Xn149Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_149_fenwick_single_element() {
        let mut ft = super::Xn149Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_149_fenwick_find_kth() {
        let mut ft = super::Xn149Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_149_fenwick_negative_delta() {
        let mut ft = super::Xn149Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 149 ----

    #[test]
    fn xn_149_avl_insert_get() {
        let mut m = super::Xn149AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_149_avl_remove() {
        let mut m = super::Xn149AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_149_avl_in_order() {
        let mut m = super::Xn149AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_149_avl_min_max() {
        let mut m = super::Xn149AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_149_avl_floor_ceiling() {
        let mut m = super::Xn149AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_149_avl_height_balanced() {
        let mut m = super::Xn149AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_149_avl_overwrite() {
        let mut m = super::Xn149AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_149_avl_empty() {
        let m: super::Xn149AVL<i32, i32> = super::Xn149AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo149RedBlack tests ---

    #[test]
    fn xo_149_rb_insert_and_get() {
        let mut tree = super::Xo149RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_149_rb_len_and_empty() {
        let mut tree = super::Xo149RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_149_rb_min_max() {
        let mut tree = super::Xo149RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_149_rb_contains() {
        let mut tree = super::Xo149RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_149_rb_remove() {
        let mut tree = super::Xo149RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_149_rb_in_order() {
        let mut tree = super::Xo149RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_149_rb_black_height() {
        let mut tree = super::Xo149RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_149_rb_overwrite() {
        let mut tree = super::Xo149RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo149ConsistentHash tests ---

    #[test]
    fn xo_149_ch_add_and_count() {
        let mut ring = super::Xo149ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_149_ch_remove_node() {
        let mut ring = super::Xo149ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_149_ch_get_node() {
        let mut ring = super::Xo149ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_149_ch_empty_ring() {
        let ring = super::Xo149ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_149_ch_distribution() {
        let mut ring = super::Xo149ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_149_ch_rebalance() {
        let mut ring = super::Xo149ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_149_ch_virtual_nodes() {
        let mut ring = super::Xo149ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_149_ch_consistent_lookup() {
        let mut ring = super::Xo149ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_149_splay_insert_get() {
        let mut t = super::Xp149SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_149_splay_remove() {
        let mut t = super::Xp149SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_149_splay_count_increases() {
        let mut t = super::Xp149SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_149_splay_depth() {
        let mut t = super::Xp149SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_149_splay_len_empty() {
        let t = super::Xp149SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_149_splay_min_max() {
        let mut t = super::Xp149SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_149_splay_overwrite() {
        let mut t = super::Xp149SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_149_splay_remove_missing() {
        let mut t = super::Xp149SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
