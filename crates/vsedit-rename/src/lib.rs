//! Rename symbol support.
//!
//! Provides the rename workflow: prepare → validate → compute edits → apply.

use std::fmt;
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

/// Result of preparing a rename — the range and placeholder text.
#[derive(Debug, Clone)]
pub struct PrepareRenameResult {
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub placeholder: String,
}

/// Result of performing a rename across the workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceEdit {
    pub edits: Vec<RenameEdit>,
}

impl WorkspaceEdit {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    /// Number of files affected by this edit.
    pub fn affected_file_count(&self) -> usize {
        let mut uris: Vec<&str> = self.edits.iter().map(|e| e.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris.len()
    }

    /// Total number of individual edits.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
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

    /// Prepare rename: ask providers if rename is valid at this position.
    pub fn prepare(&self, uri: &str, line: u32, column: u32) -> Option<PrepareRenameResult> {
        for provider in &self.providers {
            if let Some(result) = provider.prepare_rename(uri, line, column) {
                return Some(result);
            }
        }
        None
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

    /// Compute edits for the rename.
    pub fn compute_edits(
        &self,
        uri: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<WorkspaceEdit, RenameError> {
        Self::validate_new_name(new_name)?;
        for provider in &self.providers {
            if let Some(edits) = provider.provide_rename_edits(uri, line, column, new_name) {
                return Ok(edits);
            }
        }
        Err(RenameError::NoProvider)
    }
}

impl Default for RenameService {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    EmptyName,
    WhitespaceOnly,
    ContainsNewline,
    NoProvider,
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "New name cannot be empty"),
            Self::WhitespaceOnly => write!(f, "New name cannot be whitespace only"),
            Self::ContainsNewline => write!(f, "New name cannot contain newlines"),
            Self::NoProvider => write!(f, "No rename provider available"),
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
    }

    /// Sort edits by (uri, line, start_column) for deterministic application.
    pub fn sort_edits(&mut self) {
        self.edits
            .sort_by(|a, b| (&a.uri, a.line, a.start_column).cmp(&(&b.uri, b.line, b.start_column)));
    }

    /// Return all unique file URIs touched by these edits.
    pub fn affected_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.edits.iter().map(|e| e.uri.clone()).collect();
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
        };
        let we2 = WorkspaceEdit {
            edits: vec![
                RenameEdit { uri: "a.rs".into(), line: 1, start_column: 0, end_column: 2, new_text: "y".into() },
            ],
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
}
