//! Quick fix and refactoring.

use std::collections::HashMap;
use std::fmt;

/// The kind of a code action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CodeActionKind {
    QuickFix,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
    Source,
    SourceOrganizeImports,
    SourceFixAll,
}

impl CodeActionKind {
    /// Return the string identifier for this kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QuickFix => "quickfix",
            Self::Refactor => "refactor",
            Self::RefactorExtract => "refactor.extract",
            Self::RefactorInline => "refactor.inline",
            Self::RefactorRewrite => "refactor.rewrite",
            Self::Source => "source",
            Self::SourceOrganizeImports => "source.organizeImports",
            Self::SourceFixAll => "source.fixAll",
        }
    }

    /// Check if this kind contains another (hierarchical match).
    pub fn contains(&self, other: &CodeActionKind) -> bool {
        other.as_str().starts_with(self.as_str())
    }

    /// Parse a kind string back to an enum variant.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "quickfix" => Some(Self::QuickFix),
            "refactor" => Some(Self::Refactor),
            "refactor.extract" => Some(Self::RefactorExtract),
            "refactor.inline" => Some(Self::RefactorInline),
            "refactor.rewrite" => Some(Self::RefactorRewrite),
            "source" => Some(Self::Source),
            "source.organizeImports" => Some(Self::SourceOrganizeImports),
            "source.fixAll" => Some(Self::SourceFixAll),
            _ => None,
        }
    }
}

/// A single text edit within a workspace edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

/// Multi-file workspace edit: maps URI → list of text edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEdit {
    pub changes: HashMap<String, Vec<TextEdit>>,
}

impl WorkspaceEdit {
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Create from a single file URI and its edits.
    pub fn single_file(uri: impl Into<String>, edits: Vec<TextEdit>) -> Self {
        let mut changes = HashMap::new();
        changes.insert(uri.into(), edits);
        Self { changes }
    }

    /// Add a text edit for a specific file.
    pub fn add_edit(&mut self, uri: impl Into<String>, edit: TextEdit) {
        self.changes.entry(uri.into()).or_default().push(edit);
    }

    /// Number of files affected.
    pub fn file_count(&self) -> usize {
        self.changes.len()
    }

    /// Total number of edits across all files.
    pub fn edit_count(&self) -> usize {
        self.changes.values().map(|v| v.len()).sum()
    }

    /// Whether this edit is empty.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

impl Default for WorkspaceEdit {
    fn default() -> Self {
        Self::new()
    }
}

/// A command that can be executed after applying a code action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command: String,
    pub arguments: Vec<String>,
}

impl Command {
    pub fn new(title: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            command: command.into(),
            arguments: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.arguments = args;
        self
    }
}

/// A diagnostic associated with a code action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub severity: DiagnosticSeverity,
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A code action (quick fix, refactoring, etc.).
#[derive(Debug, Clone)]
pub struct CodeAction {
    pub title: String,
    pub kind: CodeActionKind,
    pub diagnostics: Vec<Diagnostic>,
    pub edit: Option<WorkspaceEdit>,
    pub command: Option<Command>,
    pub is_preferred: bool,
    pub disabled_reason: Option<String>,
}

impl CodeAction {
    pub fn new(title: impl Into<String>, kind: CodeActionKind) -> Self {
        Self {
            title: title.into(),
            kind,
            diagnostics: Vec::new(),
            edit: None,
            command: None,
            is_preferred: false,
            disabled_reason: None,
        }
    }

    pub fn with_edit(mut self, edit: WorkspaceEdit) -> Self {
        self.edit = Some(edit);
        self
    }

    pub fn with_command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    pub fn with_diagnostic(mut self, diag: Diagnostic) -> Self {
        self.diagnostics.push(diag);
        self
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.disabled_reason.is_none()
    }
}

/// Context passed to a code action provider.
#[derive(Debug, Clone)]
pub struct CodeActionContext {
    pub diagnostics: Vec<Diagnostic>,
    pub only: Option<Vec<CodeActionKind>>,
    pub trigger: CodeActionTrigger,
}

impl CodeActionContext {
    pub fn new(trigger: CodeActionTrigger) -> Self {
        Self {
            diagnostics: Vec::new(),
            only: None,
            trigger,
        }
    }
}

/// Provider of code actions.
pub trait CodeActionProvider: Send + Sync {
    fn provide_code_actions(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
        context: &CodeActionContext,
    ) -> Vec<CodeAction>;
}

/// Service that manages code action providers and dispatches requests.
pub struct CodeActionService {
    providers: Vec<Box<dyn CodeActionProvider>>,
}

impl CodeActionService {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn CodeActionProvider>) {
        self.providers.push(provider);
    }

    /// Get code actions from all providers for a given range.
    pub fn get_code_actions(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
        context: &CodeActionContext,
    ) -> Vec<CodeAction> {
        let mut all = Vec::new();
        for provider in &self.providers {
            all.extend(provider.provide_code_actions(uri, start_line, end_line, context));
        }
        // Filter by `only` kinds if specified.
        if let Some(ref only) = context.only {
            all.retain(|a| only.iter().any(|k| k.contains(&a.kind)));
        }
        all
    }

    /// Whether any code action is available (for light bulb indicator).
    pub fn has_actions(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
        context: &CodeActionContext,
    ) -> bool {
        !self.get_code_actions(uri, start_line, end_line, context).is_empty()
    }
}

impl Default for CodeActionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a code action: apply its workspace edit and return the command to run (if any).
pub fn apply_code_action(
    files: &mut HashMap<String, String>,
    action: &CodeAction,
) -> Option<Command> {
    if let Some(ref edit) = action.edit {
        apply_workspace_edit(files, edit);
    }
    action.command.clone()
}

/// Apply a workspace edit to in-memory file contents.
pub fn apply_workspace_edit(files: &mut HashMap<String, String>, edit: &WorkspaceEdit) {
    for (uri, edits) in &edit.changes {
        if let Some(content) = files.get(uri) {
            let new_content = apply_edits_to_text(content, edits);
            files.insert(uri.clone(), new_content);
        }
    }
}

/// Apply text edits to a string, applying in reverse order to preserve positions.
fn apply_edits_to_text(text: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.start_line.cmp(&a.start_line).then(b.start_col.cmp(&a.start_col))
    });

    let lines: Vec<&str> = text.lines().collect();
    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for edit in sorted {
        if edit.start_line == edit.end_line {
            let idx = edit.start_line as usize;
            if idx < result_lines.len() {
                let line = &result_lines[idx];
                let start = edit.start_col as usize;
                let end = edit.end_col as usize;
                let new_line = format!(
                    "{}{}{}",
                    &line[..start.min(line.len())],
                    edit.new_text,
                    &line[end.min(line.len())..]
                );
                result_lines[idx] = new_line;
            }
        }
    }

    result_lines.join("\n")
}

/// Filter code actions by kind.
pub fn filter_by_kind(actions: &[CodeAction], kind: &CodeActionKind) -> Vec<CodeAction> {
    actions
        .iter()
        .filter(|a| kind.contains(&a.kind))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// CodeActionSet
// ---------------------------------------------------------------------------

/// Groups a set of code actions by kind with convenience accessors.
#[derive(Debug, Clone)]
pub struct CodeActionSet {
    actions: Vec<CodeAction>,
}

impl CodeActionSet {
    pub fn from_actions(actions: Vec<CodeAction>) -> Self {
        Self { actions }
    }

    /// Returns the first preferred action, if any.
    pub fn get_preferred(&self) -> Option<&CodeAction> {
        self.actions.iter().find(|a| a.is_preferred)
    }

    /// Returns actions matching the given kind (hierarchical).
    pub fn get_by_kind(&self, kind: &CodeActionKind) -> Vec<&CodeAction> {
        self.actions
            .iter()
            .filter(|a| kind.contains(&a.kind))
            .collect()
    }

    /// Returns a new set with actions sorted by preferred-first, kind string, then title.
    pub fn sorted(&self) -> Self {
        let mut actions = self.actions.clone();
        sort_actions(&mut actions);
        Self { actions }
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, CodeAction> {
        self.actions.iter()
    }
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

/// Sorts actions: preferred first, then by kind string, then by title.
pub fn sort_actions(actions: &mut [CodeAction]) {
    actions.sort_by(|a, b| {
        b.is_preferred
            .cmp(&a.is_preferred)
            .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
            .then_with(|| a.title.cmp(&b.title))
    });
}

// ---------------------------------------------------------------------------
// Refactoring preview
// ---------------------------------------------------------------------------

/// Preview of changes a refactoring would produce, showing old vs new lines.
#[derive(Debug, Clone)]
pub struct RefactoringPreview {
    pub file_diffs: HashMap<String, Vec<(u32, String, String)>>,
}

impl RefactoringPreview {
    /// Build a preview from a workspace edit and file contents.
    pub fn from_edit(files: &HashMap<String, String>, edit: &WorkspaceEdit) -> Self {
        let mut file_diffs: HashMap<String, Vec<(u32, String, String)>> = HashMap::new();

        for (uri, edits) in &edit.changes {
            if let Some(content) = files.get(uri) {
                let lines: Vec<&str> = content.lines().collect();
                let mut diffs = Vec::new();
                for te in edits {
                    if te.start_line == te.end_line {
                        let idx = te.start_line as usize;
                        if idx < lines.len() {
                            let old = lines[idx].to_string();
                            let s = te.start_col as usize;
                            let e = te.end_col as usize;
                            let new = format!(
                                "{}{}{}",
                                &old[..s.min(old.len())],
                                te.new_text,
                                &old[e.min(old.len())..]
                            );
                            diffs.push((te.start_line, old, new));
                        }
                    }
                }
                file_diffs.insert(uri.clone(), diffs);
            }
        }

        Self { file_diffs }
    }

    /// Total number of changed lines.
    pub fn total_changes(&self) -> usize {
        self.file_diffs.values().map(|v| v.len()).sum()
    }

    /// Number of files affected.
    pub fn file_count(&self) -> usize {
        self.file_diffs.len()
    }
}

// ---------------------------------------------------------------------------
// Organize Imports
// ---------------------------------------------------------------------------

/// Sort and deduplicate import lines in source text.
/// Recognizes lines starting with `use ` or `import `.
pub fn organize_imports(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut import_block: Vec<String> = Vec::new();
    let mut in_imports = false;

    for line in &lines {
        let trimmed = line.trim();
        let is_import = trimmed.starts_with("use ") || trimmed.starts_with("import ");
        if is_import {
            import_block.push(line.to_string());
            in_imports = true;
        } else {
            if in_imports && !import_block.is_empty() {
                import_block.sort();
                import_block.dedup();
                result.extend(import_block.drain(..));
                in_imports = false;
            }
            result.push(line.to_string());
        }
    }
    // Flush trailing imports.
    if !import_block.is_empty() {
        import_block.sort();
        import_block.dedup();
        result.extend(import_block);
    }

    result.join("\n")
}

// ---------------------------------------------------------------------------
// Builder & helper extensions
// ---------------------------------------------------------------------------

impl CodeAction {
    /// Builder: mark this action as disabled with a reason.
    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Returns `true` if this action has at least one diagnostic.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Number of edits (0 or 1 since we store a single optional edit).
    pub fn edit_count(&self) -> usize {
        usize::from(self.edit.is_some())
    }
}

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

/// How a code action request was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionTrigger {
    /// Explicitly invoked by the user (e.g. Ctrl+.).
    Invoke,
    /// Automatically triggered (e.g. on save).
    Automatic,
}

/// Accumulated statistics for codeaction operations.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeactionStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl CodeactionStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &CodeactionStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for CodeactionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CodeactionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CodeactionStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for codeaction.
#[derive(Debug, Clone)]
pub struct CodeactionValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl CodeactionValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for CodeactionValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_action_creation() {
        let action = CodeAction::new("Fix typo", CodeActionKind::QuickFix)
            .preferred()
            .with_edit(WorkspaceEdit::single_file("file:///test.rs", vec![
                TextEdit {
                    start_line: 1, start_col: 0, end_line: 1, end_col: 5,
                    new_text: "fixed".into(),
                },
            ]));
        assert_eq!(action.title, "Fix typo");
        assert!(action.is_preferred);
        assert!(action.is_enabled());
        assert!(action.edit.is_some());
    }

    #[test]
    fn kind_hierarchy() {
        assert!(CodeActionKind::Refactor.contains(&CodeActionKind::RefactorExtract));
        assert!(CodeActionKind::Refactor.contains(&CodeActionKind::RefactorInline));
        assert!(!CodeActionKind::QuickFix.contains(&CodeActionKind::Refactor));
    }

    #[test]
    fn filter_actions() {
        let actions = vec![
            CodeAction::new("Fix", CodeActionKind::QuickFix),
            CodeAction::new("Extract", CodeActionKind::RefactorExtract),
            CodeAction::new("Inline", CodeActionKind::RefactorInline),
        ];
        let refactors = filter_by_kind(&actions, &CodeActionKind::Refactor);
        assert_eq!(refactors.len(), 2);
    }

    #[test]
    fn kind_strings() {
        assert_eq!(CodeActionKind::QuickFix.as_str(), "quickfix");
        assert_eq!(CodeActionKind::SourceOrganizeImports.as_str(), "source.organizeImports");
    }

    #[test]
    fn kind_from_str() {
        assert_eq!(CodeActionKind::from_str("quickfix"), Some(CodeActionKind::QuickFix));
        assert_eq!(CodeActionKind::from_str("refactor.extract"), Some(CodeActionKind::RefactorExtract));
        assert_eq!(CodeActionKind::from_str("unknown"), None);
    }

    #[test]
    fn code_action_set_preferred() {
        let actions = vec![
            CodeAction::new("A", CodeActionKind::QuickFix),
            CodeAction::new("B", CodeActionKind::Refactor).preferred(),
        ];
        let set = CodeActionSet::from_actions(actions);
        assert_eq!(set.get_preferred().unwrap().title, "B");
    }

    #[test]
    fn code_action_set_by_kind() {
        let actions = vec![
            CodeAction::new("Fix", CodeActionKind::QuickFix),
            CodeAction::new("Extract", CodeActionKind::RefactorExtract),
            CodeAction::new("Inline", CodeActionKind::RefactorInline),
        ];
        let set = CodeActionSet::from_actions(actions);
        assert_eq!(set.get_by_kind(&CodeActionKind::Refactor).len(), 2);
        assert_eq!(set.get_by_kind(&CodeActionKind::QuickFix).len(), 1);
    }

    #[test]
    fn sort_actions_preferred_first() {
        let mut actions = vec![
            CodeAction::new("Z", CodeActionKind::Refactor),
            CodeAction::new("A", CodeActionKind::QuickFix).preferred(),
        ];
        sort_actions(&mut actions);
        assert_eq!(actions[0].title, "A");
        assert!(actions[0].is_preferred);
    }

    #[test]
    fn sort_actions_stable_by_title() {
        let mut actions = vec![
            CodeAction::new("Beta", CodeActionKind::QuickFix),
            CodeAction::new("Alpha", CodeActionKind::QuickFix),
        ];
        sort_actions(&mut actions);
        assert_eq!(actions[0].title, "Alpha");
        assert_eq!(actions[1].title, "Beta");
    }

    #[test]
    fn workspace_edit_multi_file() {
        let mut we = WorkspaceEdit::new();
        we.add_edit("a.rs", TextEdit {
            start_line: 0, start_col: 0, end_line: 0, end_col: 3,
            new_text: "bar".into(),
        });
        we.add_edit("b.rs", TextEdit {
            start_line: 1, start_col: 0, end_line: 1, end_col: 2,
            new_text: "xy".into(),
        });
        assert_eq!(we.file_count(), 2);
        assert_eq!(we.edit_count(), 2);
    }

    #[test]
    fn workspace_edit_single_file() {
        let we = WorkspaceEdit::single_file("f.rs", vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 1, new_text: "X".into() },
        ]);
        assert_eq!(we.file_count(), 1);
        assert_eq!(we.edit_count(), 1);
        assert!(!we.is_empty());
    }

    #[test]
    fn workspace_edit_empty() {
        let we = WorkspaceEdit::new();
        assert!(we.is_empty());
        assert_eq!(we.file_count(), 0);
    }

    #[test]
    fn apply_workspace_edit_test() {
        let mut files = HashMap::new();
        files.insert("test.rs".to_string(), "fn main() {\n    old_call();\n}".to_string());

        let we = WorkspaceEdit::single_file("test.rs", vec![
            TextEdit {
                start_line: 1, start_col: 4, end_line: 1, end_col: 14,
                new_text: "new_call()".into(),
            },
        ]);
        apply_workspace_edit(&mut files, &we);
        assert!(files["test.rs"].contains("new_call()"));
        assert!(!files["test.rs"].contains("old_call()"));
    }

    #[test]
    fn disabled_builder() {
        let action = CodeAction::new("Nope", CodeActionKind::Refactor)
            .disabled("not applicable");
        assert!(!action.is_enabled());
        assert_eq!(action.disabled_reason.as_deref(), Some("not applicable"));
    }

    #[test]
    fn has_diagnostics_and_edit_count() {
        let action = CodeAction::new("Fix", CodeActionKind::QuickFix)
            .with_diagnostic(Diagnostic {
                message: "err".into(),
                line: 1,
                column: 0,
                severity: DiagnosticSeverity::Error,
            })
            .with_edit(WorkspaceEdit::single_file("f", vec![
                TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 1, new_text: "x".into() },
            ]));
        assert!(action.has_diagnostics());
        assert_eq!(action.edit_count(), 1);

        let empty = CodeAction::new("Empty", CodeActionKind::Source);
        assert!(!empty.has_diagnostics());
        assert_eq!(empty.edit_count(), 0);
    }

    #[test]
    fn code_action_trigger_enum() {
        let t1 = CodeActionTrigger::Invoke;
        let t2 = CodeActionTrigger::Automatic;
        assert_ne!(t1, t2);
        assert_eq!(t1, CodeActionTrigger::Invoke);
    }

    #[test]
    fn code_action_set_sorted() {
        let actions = vec![
            CodeAction::new("Z Refactor", CodeActionKind::Refactor),
            CodeAction::new("A Fix", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("B Fix", CodeActionKind::QuickFix),
        ];
        let set = CodeActionSet::from_actions(actions).sorted();
        assert_eq!(set.iter().next().unwrap().title, "A Fix");
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn code_action_set_empty() {
        let set = CodeActionSet::from_actions(vec![]);
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(set.get_preferred().is_none());
    }

    #[test]
    fn command_creation() {
        let cmd = Command::new("Run", "editor.run").with_args(vec!["--fast".into()]);
        assert_eq!(cmd.title, "Run");
        assert_eq!(cmd.command, "editor.run");
        assert_eq!(cmd.arguments, vec!["--fast"]);
    }

    #[test]
    fn code_action_with_command() {
        let action = CodeAction::new("Fix", CodeActionKind::QuickFix)
            .with_command(Command::new("Run fix", "fix.apply"));
        assert!(action.command.is_some());
        assert_eq!(action.command.unwrap().command, "fix.apply");
    }

    #[test]
    fn code_action_service_get_actions() {
        struct TestProvider;
        impl CodeActionProvider for TestProvider {
            fn provide_code_actions(&self, _uri: &str, _s: u32, _e: u32, _ctx: &CodeActionContext) -> Vec<CodeAction> {
                vec![CodeAction::new("Test Fix", CodeActionKind::QuickFix)]
            }
        }
        let mut svc = CodeActionService::new();
        svc.register(Box::new(TestProvider));
        let ctx = CodeActionContext::new(CodeActionTrigger::Invoke);
        let actions = svc.get_code_actions("f.rs", 0, 10, &ctx);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Test Fix");
    }

    #[test]
    fn code_action_service_filter_by_only() {
        struct TestProvider;
        impl CodeActionProvider for TestProvider {
            fn provide_code_actions(&self, _: &str, _: u32, _: u32, _: &CodeActionContext) -> Vec<CodeAction> {
                vec![
                    CodeAction::new("Fix", CodeActionKind::QuickFix),
                    CodeAction::new("Extract", CodeActionKind::RefactorExtract),
                ]
            }
        }
        let mut svc = CodeActionService::new();
        svc.register(Box::new(TestProvider));
        let mut ctx = CodeActionContext::new(CodeActionTrigger::Invoke);
        ctx.only = Some(vec![CodeActionKind::QuickFix]);
        let actions = svc.get_code_actions("f.rs", 0, 10, &ctx);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Fix");
    }

    #[test]
    fn code_action_service_has_actions() {
        let svc = CodeActionService::new();
        let ctx = CodeActionContext::new(CodeActionTrigger::Invoke);
        assert!(!svc.has_actions("f.rs", 0, 10, &ctx));
    }

    #[test]
    fn apply_code_action_with_edit_and_command() {
        let mut files = HashMap::new();
        files.insert("f.rs".to_string(), "let old = 1;".to_string());

        let action = CodeAction::new("Fix", CodeActionKind::QuickFix)
            .with_edit(WorkspaceEdit::single_file("f.rs", vec![
                TextEdit { start_line: 0, start_col: 4, end_line: 0, end_col: 7, new_text: "new".into() },
            ]))
            .with_command(Command::new("Format", "editor.format"));

        let cmd = apply_code_action(&mut files, &action);
        assert!(files["f.rs"].contains("new"));
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command, "editor.format");
    }

    #[test]
    fn refactoring_preview() {
        let mut files = HashMap::new();
        files.insert("r.rs".to_string(), "let foo = 1;\nfoo + 2".to_string());

        let we = WorkspaceEdit::single_file("r.rs", vec![
            TextEdit { start_line: 0, start_col: 4, end_line: 0, end_col: 7, new_text: "bar".into() },
        ]);

        let preview = RefactoringPreview::from_edit(&files, &we);
        assert_eq!(preview.file_count(), 1);
        assert_eq!(preview.total_changes(), 1);
        let diffs = &preview.file_diffs["r.rs"];
        assert!(diffs[0].1.contains("foo"));
        assert!(diffs[0].2.contains("bar"));
    }

    #[test]
    fn organize_imports_sorts_and_deduplicates() {
        let text = "use std::io;\nuse std::fmt;\nuse std::io;\n\nfn main() {}";
        let result = organize_imports(text);
        assert!(result.starts_with("use std::fmt;"));
        // Duplicate removed
        assert_eq!(result.matches("use std::io;").count(), 1);
    }

    #[test]
    fn organize_imports_preserves_non_import_lines() {
        let text = "use b;\nuse a;\n\nfn main() {}\n\nlet x = 1;";
        let result = organize_imports(text);
        assert!(result.contains("fn main() {}"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn code_action_context_new() {
        let ctx = CodeActionContext::new(CodeActionTrigger::Automatic);
        assert_eq!(ctx.trigger, CodeActionTrigger::Automatic);
        assert!(ctx.diagnostics.is_empty());
        assert!(ctx.only.is_none());
    }

    #[test]
    fn codeaction_stats_new_defaults() {
        let stats = CodeactionStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn codeaction_stats_record_success() {
        let mut stats = CodeactionStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn codeaction_stats_record_failure() {
        let mut stats = CodeactionStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn codeaction_stats_reset() {
        let mut stats = CodeactionStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn codeaction_stats_merge() {
        let mut a = CodeactionStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = CodeactionStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn codeaction_stats_display() {
        let mut stats = CodeactionStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn codeaction_stats_default() {
        let stats = CodeactionStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn codeaction_validator_accepts_valid_name() {
        let v = CodeactionValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn codeaction_validator_rejects_empty() {
        let v = CodeactionValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn codeaction_validator_rejects_too_long() {
        let v = CodeactionValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn codeaction_validator_forbidden_prefix() {
        let v = CodeactionValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn codeaction_validator_allowed_chars() {
        let v = CodeactionValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn codeaction_validator_range() {
        let v = CodeactionValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn codeaction_sanitize_removes_control() {
        let result = CodeactionValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn codeaction_truncate_short_string() {
        assert_eq!(CodeactionValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn codeaction_truncate_long_string() {
        let result = CodeactionValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn codeaction_is_ascii_printable() {
        assert!(CodeactionValidator::is_ascii_printable("Hello World 123"));
        assert!(!CodeactionValidator::is_ascii_printable("Hello\x00World"));
    }
}
