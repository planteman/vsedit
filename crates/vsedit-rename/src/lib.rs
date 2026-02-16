//! Rename symbol support.
//!
//! Provides the rename workflow: prepare → validate → compute edits → apply.

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
}
