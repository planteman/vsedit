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

// ---------------------------------------------------------------------------
// Additional helpers
// ---------------------------------------------------------------------------

impl CodeActionSet {
    /// Returns the titles of all actions.
    pub fn titles(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.title.as_str()).collect()
    }

    /// Returns `true` if any action is marked as preferred.
    pub fn has_preferred(&self) -> bool {
        self.actions.iter().any(|a| a.is_preferred)
    }
}

impl fmt::Display for CodeActionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let preferred = if self.has_preferred() { ", has preferred" } else { "" };
        write!(f, "CodeActionSet({} actions{preferred})", self.actions.len())
    }
}

impl CodeAction {
    /// Returns `true` if this action is a quick fix.
    pub fn is_quickfix(&self) -> bool {
        self.kind == CodeActionKind::QuickFix
    }

    /// Returns `true` if this action is any refactoring kind.
    pub fn is_refactoring(&self) -> bool {
        CodeActionKind::Refactor.contains(&self.kind)
    }

    /// Returns `true` if this action has a workspace edit.
    pub fn has_edit(&self) -> bool {
        self.edit.is_some()
    }

    /// Returns `true` if this action has a command.
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }
}

impl fmt::Display for CodeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pref = if self.is_preferred { " [preferred]" } else { "" };
        write!(f, "{} ({}){pref}", self.title, self.kind.as_str())
    }
}

impl WorkspaceEdit {
    /// Returns a list of file URIs affected by this edit.
    pub fn files(&self) -> Vec<&str> {
        self.changes.keys().map(|k| k.as_str()).collect()
    }
}

impl CodeActionKind {
    /// Returns `true` if this kind is a source action (Source, SourceOrganizeImports, SourceFixAll).
    pub fn is_source(&self) -> bool {
        CodeActionKind::Source.contains(self)
    }

    /// Returns `true` if this kind is any refactoring variant.
    pub fn is_refactoring(&self) -> bool {
        CodeActionKind::Refactor.contains(self)
    }
}

// ---------------------------------------------------------------------------
// Code action prioritization
// ---------------------------------------------------------------------------

/// Priority level for ordering code actions in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CodeActionPriority {
    /// Urgent: preferred quick fixes.
    High,
    /// Normal: regular actions.
    Normal,
    /// Low: source-level actions.
    Low,
}

/// Assign a priority to a code action based on its properties.
pub fn action_priority(action: &CodeAction) -> CodeActionPriority {
    if action.is_preferred && action.kind == CodeActionKind::QuickFix {
        CodeActionPriority::High
    } else if action.kind.is_source() {
        CodeActionPriority::Low
    } else {
        CodeActionPriority::Normal
    }
}

/// Sort actions by computed priority (High first), then by title.
pub fn sort_by_priority(actions: &mut [CodeAction]) {
    actions.sort_by(|a, b| {
        action_priority(a)
            .cmp(&action_priority(b))
            .then_with(|| a.title.cmp(&b.title))
    });
}

// ---------------------------------------------------------------------------
// Code action deduplication
// ---------------------------------------------------------------------------

/// Remove duplicate code actions. Two actions are duplicates if they have
/// the same title and kind. Keeps the first occurrence; prefers preferred actions.
pub fn deduplicate_actions(actions: &[CodeAction]) -> Vec<CodeAction> {
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut result = Vec::new();
    // Sort preferred first so they win
    let mut sorted: Vec<&CodeAction> = actions.iter().collect();
    sorted.sort_by(|a, b| b.is_preferred.cmp(&a.is_preferred));
    for action in sorted {
        let key = (action.title.clone(), action.kind.as_str().to_string());
        if seen.insert(key) {
            result.push(action.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Code action applicability checking
// ---------------------------------------------------------------------------

/// Check if a code action is applicable given the available diagnostics.
/// An action is applicable if it has no required diagnostics, or at least one
/// of its diagnostics matches (by message) a diagnostic in `available`.
pub fn is_action_applicable(action: &CodeAction, available: &[Diagnostic]) -> bool {
    if action.diagnostics.is_empty() {
        return true;
    }
    action.diagnostics.iter().any(|ad| {
        available.iter().any(|d| d.message == ad.message)
    })
}

/// Filter a set of actions, keeping only those applicable to `available` diagnostics.
pub fn filter_applicable(actions: &[CodeAction], available: &[Diagnostic]) -> Vec<CodeAction> {
    actions
        .iter()
        .filter(|a| is_action_applicable(a, available))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Code action categorization
// ---------------------------------------------------------------------------

/// Category grouping for display in a menu.
#[derive(Debug, Clone)]
pub struct CodeActionCategory {
    pub label: String,
    pub actions: Vec<CodeAction>,
}

/// Group actions by their kind into labeled categories.
pub fn categorize_actions(actions: &[CodeAction]) -> Vec<CodeActionCategory> {
    let mut map: std::collections::HashMap<&str, Vec<CodeAction>> =
        std::collections::HashMap::new();
    for action in actions {
        map.entry(action.kind.as_str())
            .or_default()
            .push(action.clone());
    }
    let mut categories: Vec<CodeActionCategory> = map
        .into_iter()
        .map(|(label, actions)| CodeActionCategory {
            label: label.to_string(),
            actions,
        })
        .collect();
    categories.sort_by(|a, b| a.label.cmp(&b.label));
    categories
}

/// Collect all unique `CodeActionKind` values from a slice of actions.
pub fn unique_kinds(actions: &[CodeAction]) -> Vec<CodeActionKind> {
    let mut seen = Vec::new();
    for a in actions {
        if !seen.contains(&a.kind) {
            seen.push(a.kind.clone());
        }
    }
    seen
}

/// Return only enabled actions from a slice.
pub fn enabled_actions(actions: &[CodeAction]) -> Vec<CodeAction> {
    actions
        .iter()
        .filter(|a| a.is_enabled())
        .cloned()
        .collect()
}

/// Count how many actions of each kind appear.
pub fn count_by_kind(actions: &[CodeAction]) -> Vec<(CodeActionKind, usize)> {
    let mut counts: Vec<(CodeActionKind, usize)> = Vec::new();
    for a in actions {
        if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == a.kind) {
            entry.1 += 1;
        } else {
            counts.push((a.kind.clone(), 1));
        }
    }
    counts
}

/// Build a summary string describing the action set.
pub fn action_set_summary(actions: &[CodeAction]) -> String {
    let total = actions.len();
    let preferred = actions.iter().filter(|a| a.is_preferred).count();
    let disabled = actions.iter().filter(|a| !a.is_enabled()).count();
    format!(
        "{} actions ({} preferred, {} disabled)",
        total, preferred, disabled
    )
}

/// Find the first action matching a given title (case-insensitive).
pub fn find_action_by_title<'a>(actions: &'a [CodeAction], title: &str) -> Option<&'a CodeAction> {
    let lower = title.to_lowercase();
    actions
        .iter()
        .find(|a| a.title.to_lowercase() == lower)
}

// ---------------------------------------------------------------------------
// Diagnostic helpers
// ---------------------------------------------------------------------------

impl DiagnosticSeverity {
    /// Return a human-readable label for this severity.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "info",
            Self::Hint => "hint",
        }
    }

    /// Return a numeric level (1 = Error … 4 = Hint) for sorting.
    pub fn level(&self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Information => 3,
            Self::Hint => 4,
        }
    }

    /// Returns `true` for `Error` or `Warning`.
    pub fn is_actionable(&self) -> bool {
        matches!(self, Self::Error | Self::Warning)
    }
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl Diagnostic {
    /// Format the diagnostic as `severity(line:col): message`.
    pub fn format_short(&self) -> String {
        format!(
            "{}({}:{}): {}",
            self.severity.label(),
            self.line,
            self.column,
            self.message,
        )
    }

    /// Returns `true` when the diagnostic falls within the given line range (inclusive).
    pub fn in_line_range(&self, start: u32, end: u32) -> bool {
        self.line >= start && self.line <= end
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_short())
    }
}

// ---------------------------------------------------------------------------
// TextEdit helpers
// ---------------------------------------------------------------------------

impl TextEdit {
    /// Create a simple insertion at a position.
    pub fn insert(line: u32, col: u32, text: impl Into<String>) -> Self {
        Self {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
            new_text: text.into(),
        }
    }

    /// Create a deletion spanning a range on a single line.
    pub fn delete(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
            new_text: String::new(),
        }
    }

    /// Create a replacement on a single line.
    pub fn replace(line: u32, start_col: u32, end_col: u32, text: impl Into<String>) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
            new_text: text.into(),
        }
    }

    /// Returns `true` if this edit inserts text without removing any.
    pub fn is_insert(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col && !self.new_text.is_empty()
    }

    /// Returns `true` if this edit removes text without inserting any.
    pub fn is_delete(&self) -> bool {
        self.new_text.is_empty() && (self.start_line != self.end_line || self.start_col != self.end_col)
    }

    /// Returns `true` if this is a no-op (same position, empty new text).
    pub fn is_noop(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col && self.new_text.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CodeActionContext filtering
// ---------------------------------------------------------------------------

impl CodeActionContext {
    /// Add a diagnostic to the context.
    pub fn with_diagnostic(mut self, diag: Diagnostic) -> Self {
        self.diagnostics.push(diag);
        self
    }

    /// Restrict the context to a set of kinds.
    pub fn with_only(mut self, kinds: Vec<CodeActionKind>) -> Self {
        self.only = Some(kinds);
        self
    }

    /// Returns diagnostics that fall within the given line range (inclusive).
    pub fn diagnostics_in_range(&self, start: u32, end: u32) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.in_line_range(start, end))
            .collect()
    }

    /// Returns only error-severity diagnostics.
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .collect()
    }

    /// Returns `true` if any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Error)
    }
}

// ---------------------------------------------------------------------------
// WorkspaceEdit merging
// ---------------------------------------------------------------------------

impl WorkspaceEdit {
    /// Merge another edit into this one, appending edits per file.
    pub fn merge(&mut self, other: &WorkspaceEdit) {
        for (uri, edits) in &other.changes {
            self.changes
                .entry(uri.clone())
                .or_default()
                .extend(edits.iter().cloned());
        }
    }

    /// Return a new edit containing only changes for the given URI.
    pub fn filter_file(&self, uri: &str) -> Self {
        let mut changes = HashMap::new();
        if let Some(edits) = self.changes.get(uri) {
            changes.insert(uri.to_string(), edits.clone());
        }
        Self { changes }
    }
}

/// Partition actions into two groups: quick-fixes and everything else.
pub fn partition_quickfixes(actions: &[CodeAction]) -> (Vec<CodeAction>, Vec<CodeAction>) {
    let mut fixes = Vec::new();
    let mut rest = Vec::new();
    for a in actions {
        if a.kind == CodeActionKind::QuickFix {
            fixes.push(a.clone());
        } else {
            rest.push(a.clone());
        }
    }
    (fixes, rest)
}


// ---------------------------------------------------------------------------
// CodeActionPreferred — auto-fix selection
// ---------------------------------------------------------------------------

/// Selects the best preferred action from a set.
pub struct CodeActionPreferred;

impl CodeActionPreferred {
    pub fn find_preferred(actions: &[CodeAction]) -> Option<&CodeAction> {
        actions.iter().find(|a| a.is_preferred)
    }

    pub fn find_all_preferred(actions: &[CodeAction]) -> Vec<&CodeAction> {
        actions.iter().filter(|a| a.is_preferred).collect()
    }

    pub fn find_best_fix(actions: &[CodeAction]) -> Option<&CodeAction> {
        actions.iter().find(|a| a.is_preferred && a.kind == CodeActionKind::QuickFix)
    }

    pub fn preferred_count(actions: &[CodeAction]) -> usize {
        actions.iter().filter(|a| a.is_preferred).count()
    }
}

// ---------------------------------------------------------------------------
// CodeActionWidget — grouped display
// ---------------------------------------------------------------------------

/// A group of code actions with the same kind.
pub struct CodeActionGroup {
    pub kind: CodeActionKind,
    pub actions: Vec<CodeAction>,
}

/// Groups code actions by kind for display in a widget.
pub struct CodeActionWidget {
    groups: Vec<CodeActionGroup>,
}

impl CodeActionWidget {
    pub fn from_actions(actions: Vec<CodeAction>) -> Self {
        let mut map: HashMap<String, Vec<CodeAction>> = HashMap::new();
        for action in actions {
            map.entry(action.kind.as_str().to_string()).or_default().push(action);
        }
        let mut groups: Vec<CodeActionGroup> = map
            .into_iter()
            .filter_map(|(kind_str, actions)| {
                CodeActionKind::from_str(&kind_str).map(|kind| CodeActionGroup { kind, actions })
            })
            .collect();
        groups.sort_by_key(|g| g.kind.as_str());
        Self { groups }
    }

    pub fn group_count(&self) -> usize { self.groups.len() }

    pub fn total_actions(&self) -> usize {
        self.groups.iter().map(|g| g.actions.len()).sum()
    }

    pub fn actions_for_kind(&self, kind: &CodeActionKind) -> Vec<&CodeAction> {
        self.groups.iter().filter(|g| &g.kind == kind).flat_map(|g| g.actions.iter()).collect()
    }

    pub fn groups(&self) -> &[CodeActionGroup] { &self.groups }

    pub fn first_action(&self) -> Option<&CodeAction> {
        self.groups.first().and_then(|g| g.actions.first())
    }
}

impl fmt::Display for CodeActionWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CodeActionWidget({} groups, {} actions)", self.group_count(), self.total_actions())
    }
}

// ---------------------------------------------------------------------------
// CodeActionKeybinding — quick access
// ---------------------------------------------------------------------------

/// Associates a keybinding with a code action kind.
#[derive(Debug, Clone)]
pub struct CodeActionKeybinding {
    pub key: String,
    pub kind: CodeActionKind,
    pub when_clause: Option<String>,
}

impl CodeActionKeybinding {
    pub fn new(key: impl Into<String>, kind: CodeActionKind) -> Self {
        Self { key: key.into(), kind, when_clause: None }
    }

    pub fn with_when(mut self, when: impl Into<String>) -> Self {
        self.when_clause = Some(when.into());
        self
    }

    pub fn matches_key(&self, key: &str) -> bool { self.key == key }
}

impl fmt::Display for CodeActionKeybinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.key, self.kind.as_str())
    }
}

/// Manages keybindings for code actions.
pub struct CodeActionKeybindingRegistry {
    bindings: Vec<CodeActionKeybinding>,
}

impl CodeActionKeybindingRegistry {
    pub fn new() -> Self { Self { bindings: Vec::new() } }

    pub fn register(&mut self, binding: CodeActionKeybinding) { self.bindings.push(binding); }

    pub fn find_by_key(&self, key: &str) -> Option<&CodeActionKeybinding> {
        self.bindings.iter().find(|b| b.matches_key(key))
    }

    pub fn len(&self) -> usize { self.bindings.len() }
    pub fn is_empty(&self) -> bool { self.bindings.is_empty() }
}

impl Default for CodeActionKeybindingRegistry {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// CodeActionSourceFilter — filter by source
// ---------------------------------------------------------------------------

/// Filters code actions by source kind.
pub struct CodeActionSourceFilter {
    allowed_kinds: Vec<CodeActionKind>,
}

impl CodeActionSourceFilter {
    pub fn allow_all() -> Self { Self { allowed_kinds: vec![] } }

    pub fn only(kinds: Vec<CodeActionKind>) -> Self { Self { allowed_kinds: kinds } }

    pub fn apply<'a>(&self, actions: &'a [CodeAction]) -> Vec<&'a CodeAction> {
        if self.allowed_kinds.is_empty() {
            return actions.iter().collect();
        }
        actions.iter().filter(|a| self.allowed_kinds.contains(&a.kind)).collect()
    }

    pub fn allowed_count(&self) -> usize { self.allowed_kinds.len() }

    pub fn is_allowed(&self, kind: &CodeActionKind) -> bool {
        self.allowed_kinds.is_empty() || self.allowed_kinds.contains(kind)
    }
}

impl fmt::Display for CodeActionSourceFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.allowed_kinds.is_empty() {
            write!(f, "CodeActionSourceFilter(all)")
        } else {
            write!(f, "CodeActionSourceFilter({} kinds)", self.allowed_kinds.len())
        }
    }
}

// ---------------------------------------------------------------------------
// CodeActionPreferredPicker - code action preferred picker
// ---------------------------------------------------------------------------

/// Severity level for code action preferred picker issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodeActionPreferredPickerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for CodeActionPreferredPickerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [CodeActionPreferredPicker].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionPreferredPickerEntry {
    pub id: String,
    pub label: String,
    pub severity: CodeActionPreferredPickerSeverity,
    pub detail: Option<String>,
    pub action_count: usize,
    enabled: bool,
}

impl CodeActionPreferredPickerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: CodeActionPreferredPickerSeverity::Low,
            detail: None,
            action_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: CodeActionPreferredPickerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_action_count(mut self, val: usize) -> Self {
        self.action_count = val;
        self
    }

    pub fn has_preferred(&self) -> bool {
        self.enabled && self.severity >= CodeActionPreferredPickerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.action_count, det)
    }
}

impl fmt::Display for CodeActionPreferredPickerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [CodeActionPreferredPickerEntry] items.
#[derive(Debug, Clone)]
pub struct CodeActionPreferredPicker {
    entries: Vec<CodeActionPreferredPickerEntry>,
    name: String,
    capacity: usize,
}

impl CodeActionPreferredPicker {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: CodeActionPreferredPickerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<CodeActionPreferredPickerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&CodeActionPreferredPickerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn action_count(&self) -> usize { self.entries.len() }

    pub fn has_preferred(&self) -> bool {
        self.entries.iter().any(|e| e.has_preferred())
    }

    pub fn entries_by_severity(&self, severity: CodeActionPreferredPickerSeverity) -> Vec<&CodeActionPreferredPickerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= CodeActionPreferredPickerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&CodeActionPreferredPickerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&CodeActionPreferredPickerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// CodeActionQuickApply - code action quick apply
// ---------------------------------------------------------------------------

/// Configuration for [CodeActionQuickApply].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionQuickApplyConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub preferred_count: usize,
}

impl CodeActionQuickApplyConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, preferred_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_preferred_count(mut self, val: usize) -> Self { self.preferred_count = val; self }
}

impl Default for CodeActionQuickApplyConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [CodeActionQuickApply].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionQuickApplyItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl CodeActionQuickApplyItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn can_quick_apply(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for CodeActionQuickApplyItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [CodeActionQuickApplyItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct CodeActionQuickApply {
    config: CodeActionQuickApplyConfig,
    items: Vec<CodeActionQuickApplyItem>,
}

impl CodeActionQuickApply {
    pub fn new(config: CodeActionQuickApplyConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: CodeActionQuickApplyItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<CodeActionQuickApplyItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&CodeActionQuickApplyItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn preferred_count(&self) -> usize { self.items.len() }

    pub fn can_quick_apply(&self) -> bool {
        self.items.iter().any(|i| i.can_quick_apply())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&CodeActionQuickApplyItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&CodeActionQuickApplyItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &CodeActionQuickApplyConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ─── CodeAct Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for code actions.
#[derive(Debug, Clone)]
pub struct CodeActRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> CodeActRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for CodeActRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CodeActRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── CodeAct Formatter ───────────────────────────────────────

/// Formatting options for code action output.
#[derive(Debug, Clone)]
pub struct CodeActFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for CodeActFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl CodeActFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for code action data.
pub struct CodeActFmt {
    options: CodeActFmtOpts,
}

impl CodeActFmt {
    pub fn new(options: CodeActFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: CodeActFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}



// ---------------------------------------------------------------------------
// codeaction – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for code action provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YCodeactionCodeActionPriority {
    Low,
    Normal,
    High,
    Preferred,
}

impl YCodeactionCodeActionPriority {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Preferred => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Preferred => "Preferred",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YCodeactionCodeActionPriority] {
        &[
            YCodeactionCodeActionPriority::Low,
            YCodeactionCodeActionPriority::Normal,
            YCodeactionCodeActionPriority::High,
            YCodeactionCodeActionPriority::Preferred,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YCodeactionCodeActionPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks code action batch data.
#[derive(Debug, Clone)]
pub struct YCodeactionCodeActionBatch {
    pub actions: Vec<(String, bool)>,
    pub source_uri: String,
    pub line: u32,
}

impl YCodeactionCodeActionBatch {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            source_uri: String::new(),
            line: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.actions.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YCodeactionCodeActionBatch({}: {:?})", "actions", self.actions)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_codeaction_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_codeaction_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_codeaction_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_codeaction_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_codeaction_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_codeaction_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_codeaction_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_codeaction_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// codeaction – Extended code action filter helpers
// ---------------------------------------------------------------------------

/// Priority levels for code action filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZCodeactionPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZCodeactionPriority {
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
    pub fn all_asc() -> [ZCodeactionPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZCodeactionPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks code action filter data.
#[derive(Debug, Clone)]
pub struct ZCodeactionCodeActionFilter {
    pub kind_patterns: Vec<String>,
    pub include_disabled: bool,
    pub max_results: usize,
}

impl ZCodeactionCodeActionFilter {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            kind_patterns: Vec::new(),
            include_disabled: false,
            max_results: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.kind_patterns.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.kind_patterns.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.kind_patterns.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZCodeactionCodeActionFilter[include_disabled={:?}, max_results={:?}]", self.include_disabled, self.max_results)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for code action filter.
pub fn z_codeaction_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_codeaction_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_codeaction_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_codeaction_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_codeaction_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_codeaction_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_codeaction_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 72
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer72 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer72 {
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
pub fn xb_fnv1a_72(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_72<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_72<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_72(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_72(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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

    #[test]
    fn code_action_set_titles() {
        let set = CodeActionSet::from_actions(vec![
            CodeAction::new("Fix A", CodeActionKind::QuickFix),
            CodeAction::new("Extract B", CodeActionKind::RefactorExtract),
        ]);
        let titles = set.titles();
        assert_eq!(titles, vec!["Fix A", "Extract B"]);
    }

    #[test]
    fn code_action_set_has_preferred() {
        let set1 = CodeActionSet::from_actions(vec![
            CodeAction::new("A", CodeActionKind::QuickFix),
        ]);
        assert!(!set1.has_preferred());

        let set2 = CodeActionSet::from_actions(vec![
            CodeAction::new("B", CodeActionKind::QuickFix).preferred(),
        ]);
        assert!(set2.has_preferred());
    }

    #[test]
    fn code_action_set_display() {
        let set = CodeActionSet::from_actions(vec![
            CodeAction::new("Fix", CodeActionKind::QuickFix).preferred(),
        ]);
        let s = format!("{set}");
        assert!(s.contains("1 actions"));
        assert!(s.contains("has preferred"));
    }

    #[test]
    fn code_action_is_quickfix_and_refactoring() {
        let qf = CodeAction::new("Fix", CodeActionKind::QuickFix);
        assert!(qf.is_quickfix());
        assert!(!qf.is_refactoring());

        let rf = CodeAction::new("Extract", CodeActionKind::RefactorExtract);
        assert!(!rf.is_quickfix());
        assert!(rf.is_refactoring());
    }

    #[test]
    fn code_action_has_edit_and_command() {
        let a = CodeAction::new("A", CodeActionKind::QuickFix);
        assert!(!a.has_edit());
        assert!(!a.has_command());

        let b = CodeAction::new("B", CodeActionKind::QuickFix)
            .with_edit(WorkspaceEdit::new())
            .with_command(Command::new("cmd", "cmd.id"));
        assert!(b.has_edit());
        assert!(b.has_command());
    }

    #[test]
    fn code_action_display() {
        let a = CodeAction::new("Fix typo", CodeActionKind::QuickFix).preferred();
        let s = format!("{a}");
        assert!(s.contains("Fix typo"));
        assert!(s.contains("quickfix"));
        assert!(s.contains("[preferred]"));
    }

    #[test]
    fn workspace_edit_files() {
        let mut edit = WorkspaceEdit::new();
        edit.add_edit("file:///a.rs", TextEdit {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            new_text: "x".into(),
        });
        edit.add_edit("file:///b.rs", TextEdit {
            start_line: 0, start_col: 0, end_line: 0, end_col: 1,
            new_text: "y".into(),
        });
        let files = edit.files();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"file:///a.rs"));
        assert!(files.contains(&"file:///b.rs"));
    }

    #[test]
    fn code_action_kind_is_source() {
        assert!(CodeActionKind::Source.is_source());
        assert!(CodeActionKind::SourceOrganizeImports.is_source());
        assert!(CodeActionKind::SourceFixAll.is_source());
        assert!(!CodeActionKind::QuickFix.is_source());
        assert!(!CodeActionKind::Refactor.is_source());
    }

    #[test]
    fn code_action_kind_is_refactoring() {
        assert!(CodeActionKind::Refactor.is_refactoring());
        assert!(CodeActionKind::RefactorExtract.is_refactoring());
        assert!(CodeActionKind::RefactorInline.is_refactoring());
        assert!(!CodeActionKind::QuickFix.is_refactoring());
        assert!(!CodeActionKind::Source.is_refactoring());
    }

    #[test]
    fn action_priority_preferred_quickfix_is_high() {
        let a = CodeAction::new("Fix", CodeActionKind::QuickFix).preferred();
        assert_eq!(action_priority(&a), CodeActionPriority::High);
    }

    #[test]
    fn action_priority_source_is_low() {
        let a = CodeAction::new("Organize", CodeActionKind::SourceOrganizeImports);
        assert_eq!(action_priority(&a), CodeActionPriority::Low);
    }

    #[test]
    fn action_priority_regular_is_normal() {
        let a = CodeAction::new("Extract", CodeActionKind::RefactorExtract);
        assert_eq!(action_priority(&a), CodeActionPriority::Normal);
    }

    #[test]
    fn deduplicate_actions_removes_dupes() {
        let actions = vec![
            CodeAction::new("Fix A", CodeActionKind::QuickFix),
            CodeAction::new("Fix A", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("Fix B", CodeActionKind::QuickFix),
        ];
        let deduped = deduplicate_actions(&actions);
        assert_eq!(deduped.len(), 2);
        // The preferred version should win
        let fix_a = deduped.iter().find(|a| a.title == "Fix A").unwrap();
        assert!(fix_a.is_preferred);
    }

    #[test]
    fn is_action_applicable_no_diagnostics() {
        let a = CodeAction::new("Fix", CodeActionKind::QuickFix);
        assert!(is_action_applicable(&a, &[]));
    }

    #[test]
    fn is_action_applicable_matching_diag() {
        let diag = Diagnostic {
            message: "unused var".into(),
            severity: DiagnosticSeverity::Warning,
            line: 1,
            column: 0,
        };
        let a = CodeAction::new("Remove", CodeActionKind::QuickFix)
            .with_diagnostic(diag.clone());
        assert!(is_action_applicable(&a, &[diag]));
    }

    #[test]
    fn is_action_applicable_no_match() {
        let diag = Diagnostic {
            message: "unused var".into(),
            severity: DiagnosticSeverity::Warning,
            line: 1,
            column: 0,
        };
        let other = Diagnostic {
            message: "type error".into(),
            severity: DiagnosticSeverity::Error,
            line: 2,
            column: 0,
        };
        let a = CodeAction::new("Remove", CodeActionKind::QuickFix)
            .with_diagnostic(diag);
        assert!(!is_action_applicable(&a, &[other]));
    }

    #[test]
    fn categorize_actions_groups_by_kind() {
        let actions = vec![
            CodeAction::new("Fix A", CodeActionKind::QuickFix),
            CodeAction::new("Fix B", CodeActionKind::QuickFix),
            CodeAction::new("Extract", CodeActionKind::RefactorExtract),
        ];
        let cats = categorize_actions(&actions);
        assert_eq!(cats.len(), 2);
        let qf = cats.iter().find(|c| c.label == "quickfix").unwrap();
        assert_eq!(qf.actions.len(), 2);
    }

    #[test]
    fn sort_by_priority_orders_correctly() {
        let mut actions = vec![
            CodeAction::new("Organize", CodeActionKind::SourceOrganizeImports),
            CodeAction::new("Fix", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("Extract", CodeActionKind::RefactorExtract),
        ];
        sort_by_priority(&mut actions);
        assert_eq!(actions[0].title, "Fix");
        assert_eq!(actions[1].title, "Extract");
        assert_eq!(actions[2].title, "Organize");
    }

    #[test]
    fn unique_kinds_returns_distinct() {
        let actions = vec![
            CodeAction::new("A", CodeActionKind::QuickFix),
            CodeAction::new("B", CodeActionKind::QuickFix),
            CodeAction::new("C", CodeActionKind::RefactorExtract),
        ];
        let kinds = unique_kinds(&actions);
        assert_eq!(kinds.len(), 2);
        assert!(kinds.contains(&CodeActionKind::QuickFix));
        assert!(kinds.contains(&CodeActionKind::RefactorExtract));
    }

    #[test]
    fn unique_kinds_empty_input() {
        assert!(unique_kinds(&[]).is_empty());
    }

    #[test]
    fn enabled_actions_filters_disabled() {
        let actions = vec![
            CodeAction::new("Ok", CodeActionKind::QuickFix),
            CodeAction::new("Nope", CodeActionKind::QuickFix).disabled("reason"),
        ];
        let result = enabled_actions(&actions);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Ok");
    }

    #[test]
    fn count_by_kind_tallies() {
        let actions = vec![
            CodeAction::new("A", CodeActionKind::QuickFix),
            CodeAction::new("B", CodeActionKind::Source),
            CodeAction::new("C", CodeActionKind::QuickFix),
        ];
        let counts = count_by_kind(&actions);
        let qf = counts.iter().find(|(k, _)| *k == CodeActionKind::QuickFix).unwrap();
        assert_eq!(qf.1, 2);
    }

    #[test]
    fn action_set_summary_format() {
        let actions = vec![
            CodeAction::new("A", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("B", CodeActionKind::QuickFix).disabled("no"),
        ];
        let s = action_set_summary(&actions);
        assert!(s.contains("2 actions"));
        assert!(s.contains("1 preferred"));
        assert!(s.contains("1 disabled"));
    }

    #[test]
    fn find_action_by_title_case_insensitive() {
        let actions = vec![
            CodeAction::new("Fix Typo", CodeActionKind::QuickFix),
        ];
        assert!(find_action_by_title(&actions, "fix typo").is_some());
        assert!(find_action_by_title(&actions, "FIX TYPO").is_some());
        assert!(find_action_by_title(&actions, "other").is_none());
    }

    #[test]
    fn partition_quickfixes_splits() {
        let actions = vec![
            CodeAction::new("Fix", CodeActionKind::QuickFix),
            CodeAction::new("Extract", CodeActionKind::RefactorExtract),
            CodeAction::new("Fix2", CodeActionKind::QuickFix),
        ];
        let (fixes, rest) = partition_quickfixes(&actions);
        assert_eq!(fixes.len(), 2);
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].title, "Extract");
    }

    #[test]
    fn partition_quickfixes_empty() {
        let (fixes, rest) = partition_quickfixes(&[]);
        assert!(fixes.is_empty());
        assert!(rest.is_empty());
    }

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn diagnostic_severity_label_and_level() {
        assert_eq!(DiagnosticSeverity::Error.label(), "error");
        assert_eq!(DiagnosticSeverity::Warning.label(), "warning");
        assert_eq!(DiagnosticSeverity::Information.label(), "info");
        assert_eq!(DiagnosticSeverity::Hint.label(), "hint");
        assert!(DiagnosticSeverity::Error.level() < DiagnosticSeverity::Hint.level());
        assert!(DiagnosticSeverity::Error.is_actionable());
        assert!(DiagnosticSeverity::Warning.is_actionable());
        assert!(!DiagnosticSeverity::Hint.is_actionable());
    }

    #[test]
    fn diagnostic_format_short_and_display() {
        let d = Diagnostic {
            message: "unused".into(),
            line: 10,
            column: 5,
            severity: DiagnosticSeverity::Warning,
        };
        assert_eq!(d.format_short(), "warning(10:5): unused");
        assert_eq!(format!("{d}"), "warning(10:5): unused");
    }

    #[test]
    fn diagnostic_in_line_range() {
        let d = Diagnostic {
            message: "x".into(), line: 5, column: 0,
            severity: DiagnosticSeverity::Error,
        };
        assert!(d.in_line_range(0, 10));
        assert!(d.in_line_range(5, 5));
        assert!(!d.in_line_range(6, 10));
    }

    #[test]
    fn text_edit_insert_delete_replace() {
        let ins = TextEdit::insert(3, 0, "hello");
        assert!(ins.is_insert());
        assert!(!ins.is_delete());

        let del = TextEdit::delete(1, 2, 8);
        assert!(del.is_delete());
        assert!(!del.is_insert());

        let rep = TextEdit::replace(0, 0, 5, "world");
        assert!(!rep.is_insert());
        assert!(!rep.is_delete());
        assert!(!rep.is_noop());

        let noop = TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 0, new_text: String::new() };
        assert!(noop.is_noop());
    }

    #[test]
    fn context_diagnostics_in_range_and_errors() {
        let ctx = CodeActionContext::new(CodeActionTrigger::Invoke)
            .with_diagnostic(Diagnostic {
                message: "e".into(), line: 2, column: 0,
                severity: DiagnosticSeverity::Error,
            })
            .with_diagnostic(Diagnostic {
                message: "w".into(), line: 8, column: 0,
                severity: DiagnosticSeverity::Warning,
            })
            .with_only(vec![CodeActionKind::QuickFix]);

        assert!(ctx.has_errors());
        assert_eq!(ctx.errors().len(), 1);
        assert_eq!(ctx.diagnostics_in_range(0, 5).len(), 1);
        assert_eq!(ctx.diagnostics_in_range(0, 10).len(), 2);
        assert!(ctx.only.is_some());
    }

    #[test]
    fn workspace_edit_merge() {
        let mut a = WorkspaceEdit::single_file("a.rs", vec![
            TextEdit::insert(0, 0, "x"),
        ]);
        let b = WorkspaceEdit::single_file("a.rs", vec![
            TextEdit::insert(1, 0, "y"),
        ]);
        a.merge(&b);
        assert_eq!(a.edit_count(), 2);
        assert_eq!(a.file_count(), 1);

        let c = WorkspaceEdit::single_file("b.rs", vec![
            TextEdit::insert(0, 0, "z"),
        ]);
        a.merge(&c);
        assert_eq!(a.file_count(), 2);
    }

    #[test]
    fn workspace_edit_filter_file() {
        let mut we = WorkspaceEdit::new();
        we.add_edit("a.rs", TextEdit::insert(0, 0, "x"));
        we.add_edit("b.rs", TextEdit::insert(0, 0, "y"));

        let filtered = we.filter_file("a.rs");
        assert_eq!(filtered.file_count(), 1);
        assert_eq!(filtered.edit_count(), 1);

        let empty = we.filter_file("c.rs");
        assert!(empty.is_empty());
    }

    #[test]
    fn diagnostic_severity_display() {
        assert_eq!(format!("{}", DiagnosticSeverity::Error), "error");
        assert_eq!(format!("{}", DiagnosticSeverity::Information), "info");
    }


    #[test]
    fn preferred_find_single() {
        let actions = vec![
            CodeAction::new("fix1", CodeActionKind::QuickFix),
            CodeAction::new("fix2", CodeActionKind::QuickFix).preferred(),
        ];
        assert_eq!(CodeActionPreferred::find_preferred(&actions).unwrap().title, "fix2");
    }

    #[test]
    fn preferred_find_all() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("b", CodeActionKind::Refactor).preferred(),
            CodeAction::new("c", CodeActionKind::Source),
        ];
        assert_eq!(CodeActionPreferred::find_all_preferred(&actions).len(), 2);
    }

    #[test]
    fn preferred_best_fix() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::Refactor).preferred(),
            CodeAction::new("b", CodeActionKind::QuickFix).preferred(),
        ];
        assert_eq!(CodeActionPreferred::find_best_fix(&actions).unwrap().title, "b");
    }

    #[test]
    fn preferred_count() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix).preferred(),
            CodeAction::new("b", CodeActionKind::Refactor),
        ];
        assert_eq!(CodeActionPreferred::preferred_count(&actions), 1);
    }

    #[test]
    fn widget_grouping() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix),
            CodeAction::new("b", CodeActionKind::QuickFix),
            CodeAction::new("c", CodeActionKind::Refactor),
        ];
        let widget = CodeActionWidget::from_actions(actions);
        assert_eq!(widget.group_count(), 2);
        assert_eq!(widget.total_actions(), 3);
    }

    #[test]
    fn widget_actions_for_kind() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix),
            CodeAction::new("b", CodeActionKind::Source),
        ];
        let widget = CodeActionWidget::from_actions(actions);
        assert_eq!(widget.actions_for_kind(&CodeActionKind::QuickFix).len(), 1);
    }

    #[test]
    fn widget_display() {
        let widget = CodeActionWidget::from_actions(vec![]);
        assert!(format!("{widget}").contains("0 groups"));
    }

    #[test]
    fn keybinding_matches() {
        let kb = CodeActionKeybinding::new("ctrl+.", CodeActionKind::QuickFix);
        assert!(kb.matches_key("ctrl+."));
        assert!(!kb.matches_key("ctrl+,"));
    }

    #[test]
    fn keybinding_registry() {
        let mut reg = CodeActionKeybindingRegistry::new();
        reg.register(CodeActionKeybinding::new("ctrl+.", CodeActionKind::QuickFix));
        reg.register(CodeActionKeybinding::new("ctrl+shift+r", CodeActionKind::Refactor));
        assert_eq!(reg.len(), 2);
        assert!(reg.find_by_key("ctrl+.").is_some());
    }

    #[test]
    fn source_filter_allow_all() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix),
            CodeAction::new("b", CodeActionKind::Refactor),
        ];
        let filter = CodeActionSourceFilter::allow_all();
        assert_eq!(filter.apply(&actions).len(), 2);
    }

    #[test]
    fn source_filter_only() {
        let actions = vec![
            CodeAction::new("a", CodeActionKind::QuickFix),
            CodeAction::new("b", CodeActionKind::Refactor),
        ];
        let filter = CodeActionSourceFilter::only(vec![CodeActionKind::QuickFix]);
        assert_eq!(filter.apply(&actions).len(), 1);
    }

    #[test]
    fn keybinding_display() {
        let kb = CodeActionKeybinding::new("ctrl+.", CodeActionKind::QuickFix);
        assert!(format!("{kb}").contains("ctrl+."));
    }

    #[test]
    fn source_filter_display() {
        let f = CodeActionSourceFilter::allow_all();
        assert!(format!("{f}").contains("all"));
    }


#[test]
    fn codeactionpreferredpicker_severity_ordering() {
        assert!(CodeActionPreferredPickerSeverity::Critical > CodeActionPreferredPickerSeverity::High);
        assert!(CodeActionPreferredPickerSeverity::High > CodeActionPreferredPickerSeverity::Medium);
        assert!(CodeActionPreferredPickerSeverity::Medium > CodeActionPreferredPickerSeverity::Low);
    }

    #[test]
    fn codeactionpreferredpicker_severity_display() {
        assert_eq!(CodeActionPreferredPickerSeverity::Low.to_string(), "low");
        assert_eq!(CodeActionPreferredPickerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn codeactionpreferredpicker_entry_creation() {
        let e = CodeActionPreferredPickerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, CodeActionPreferredPickerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn codeactionpreferredpicker_entry_builder() {
        let e = CodeActionPreferredPickerEntry::new("e2", "Entry 2")
            .with_severity(CodeActionPreferredPickerSeverity::High)
            .with_detail("some detail")
            .with_action_count(42);
        assert_eq!(e.severity, CodeActionPreferredPickerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.action_count, 42);
    }

    #[test]
    fn codeactionpreferredpicker_entry_enable_disable() {
        let mut e = CodeActionPreferredPickerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn codeactionpreferredpicker_add_and_count() {
        let mut mgr = CodeActionPreferredPicker::new("test");
        mgr.add(CodeActionPreferredPickerEntry::new("a", "A"));
        mgr.add(CodeActionPreferredPickerEntry::new("b", "B").with_severity(CodeActionPreferredPickerSeverity::High));
        assert_eq!(mgr.action_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn codeactionpreferredpicker_remove() {
        let mut mgr = CodeActionPreferredPicker::new("test");
        mgr.add(CodeActionPreferredPickerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn codeactionpreferredpicker_capacity() {
        let mut mgr = CodeActionPreferredPicker::new("test").with_capacity(1);
        assert!(mgr.add(CodeActionPreferredPickerEntry::new("a", "A")));
        assert!(!mgr.add(CodeActionPreferredPickerEntry::new("b", "B")));
    }

    #[test]
    fn codeactionpreferredpicker_sorted_by_severity() {
        let mut mgr = CodeActionPreferredPicker::new("test");
        mgr.add(CodeActionPreferredPickerEntry::new("lo", "Low"));
        mgr.add(CodeActionPreferredPickerEntry::new("hi", "High").with_severity(CodeActionPreferredPickerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, CodeActionPreferredPickerSeverity::Critical);
    }

    #[test]
    fn codeactionpreferredpicker_summary() {
        let mgr = CodeActionPreferredPicker::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn codeactionquickapply_config_defaults() {
        let cfg = CodeActionQuickApplyConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn codeactionquickapply_item_creation() {
        let item = CodeActionQuickApplyItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn codeactionquickapply_add_and_get() {
        let mut mgr = CodeActionQuickApply::new(CodeActionQuickApplyConfig::new("test"));
        mgr.add(CodeActionQuickApplyItem::new("k1", "v1"));
        assert_eq!(mgr.preferred_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn codeactionquickapply_remove_item() {
        let mut mgr = CodeActionQuickApply::new(CodeActionQuickApplyConfig::new("test"));
        mgr.add(CodeActionQuickApplyItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn codeactionquickapply_sorted_by_priority() {
        let mut mgr = CodeActionQuickApply::new(CodeActionQuickApplyConfig::new("test"));
        mgr.add(CodeActionQuickApplyItem::new("lo", "low").with_priority(1));
        mgr.add(CodeActionQuickApplyItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn codeactionquickapply_items_with_tag() {
        let mut mgr = CodeActionQuickApply::new(CodeActionQuickApplyConfig::new("test"));
        mgr.add(CodeActionQuickApplyItem::new("a", "1").with_tag("x"));
        mgr.add(CodeActionQuickApplyItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn codeactionquickapply_report() {
        let mgr = CodeActionQuickApply::new(CodeActionQuickApplyConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn codeact_ringbuf_push_get() {
        let mut rb = CodeActRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn codeact_ringbuf_overflow() {
        let mut rb = CodeActRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn codeact_ringbuf_clear() {
        let mut rb = CodeActRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn codeact_ringbuf_newest_oldest() {
        let mut rb = CodeActRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn codeact_ringbuf_to_vec() {
        let mut rb = CodeActRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn codeact_ringbuf_is_full() {
        let mut rb = CodeActRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn codeact_fmt_list() {
        let f = CodeActFmt::new(CodeActFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn codeact_fmt_kv() {
        let f = CodeActFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn codeact_fmt_section() {
        let f = CodeActFmt::new(CodeActFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn codeact_fmt_truncate() {
        let f = CodeActFmt::new(CodeActFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn codeact_fmt_opts_defaults() {
        let o = CodeActFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    // -- codeaction extended domain tests ----------------------------------------

    #[test]
    fn y_codeaction_enum_index() {
        assert_eq!(YCodeactionCodeActionPriority::Low.index(), 0);
        assert_eq!(YCodeactionCodeActionPriority::Normal.index(), 1);
        assert_eq!(YCodeactionCodeActionPriority::High.index(), 2);
        assert_eq!(YCodeactionCodeActionPriority::Preferred.index(), 3);
    }

    #[test]
    fn y_codeaction_enum_label() {
        assert_eq!(YCodeactionCodeActionPriority::Low.label(), "Low");
        assert_eq!(YCodeactionCodeActionPriority::Normal.label(), "Normal");
        assert_eq!(YCodeactionCodeActionPriority::High.label(), "High");
        assert_eq!(YCodeactionCodeActionPriority::Preferred.label(), "Preferred");
    }

    #[test]
    fn y_codeaction_enum_all() {
        let all = YCodeactionCodeActionPriority::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_codeaction_enum_is_default() {
        assert!(YCodeactionCodeActionPriority::Low.is_default());
        assert!(!YCodeactionCodeActionPriority::Preferred.is_default());
    }

    #[test]
    fn y_codeaction_enum_display() {
        assert_eq!(format!("{}", YCodeactionCodeActionPriority::Low), "Low");
    }

    #[test]
    fn y_codeaction_struct_new() {
        let s = YCodeactionCodeActionBatch::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_codeaction_struct_clear() {
        let mut s = YCodeactionCodeActionBatch::new();
        s.actions.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_codeaction_fingerprint_deterministic() {
        let h1 = y_codeaction_fingerprint("hello");
        let h2 = y_codeaction_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_codeaction_fingerprint("a"), y_codeaction_fingerprint("b"));
    }

    #[test]
    fn y_codeaction_truncate_short() {
        assert_eq!(y_codeaction_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_codeaction_truncate_long() {
        let r = y_codeaction_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_codeaction_normalize_key_basic() {
        assert_eq!(y_codeaction_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_codeaction_split_path_basic() {
        let parts = y_codeaction_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_codeaction_count_occurrences_basic() {
        assert_eq!(y_codeaction_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_codeaction_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_codeaction_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_codeaction_in_range_basic() {
        assert!(y_codeaction_in_range(5, 1, 10));
        assert!(y_codeaction_in_range(1, 1, 10));
        assert!(y_codeaction_in_range(10, 1, 10));
        assert!(!y_codeaction_in_range(0, 1, 10));
        assert!(!y_codeaction_in_range(11, 1, 10));
    }

    #[test]
    fn y_codeaction_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_codeaction_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_codeaction_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_codeaction_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- codeaction Z-extended tests -----------------------------------------------

    #[test]
    fn z_codeaction_priority_weight() {
        assert_eq!(ZCodeactionPriority::Idle.weight(), 0);
        assert_eq!(ZCodeactionPriority::Normal.weight(), 2);
        assert_eq!(ZCodeactionPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_codeaction_priority_label() {
        assert_eq!(ZCodeactionPriority::Low.label(), "low");
        assert_eq!(ZCodeactionPriority::High.label(), "high");
    }

    #[test]
    fn z_codeaction_priority_is_elevated() {
        assert!(!ZCodeactionPriority::Normal.is_elevated());
        assert!(ZCodeactionPriority::High.is_elevated());
        assert!(ZCodeactionPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_codeaction_priority_display() {
        assert_eq!(format!("{}", ZCodeactionPriority::Idle), "idle");
    }

    #[test]
    fn z_codeaction_priority_all_asc() {
        let all = ZCodeactionPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZCodeactionPriority::Idle);
        assert_eq!(all[4], ZCodeactionPriority::Realtime);
    }

    #[test]
    fn z_codeaction_struct_new() {
        let s = ZCodeactionCodeActionFilter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_codeaction_struct_toggled_clone() {
        let s = ZCodeactionCodeActionFilter::new();
        let t = s.toggled_clone();
        let _ = t.max_results;
    }

    #[test]
    fn z_codeaction_rolling_hash_deterministic() {
        let h1 = z_codeaction_rolling_hash(b"test");
        let h2 = z_codeaction_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_codeaction_rolling_hash(b"a"), z_codeaction_rolling_hash(b"b"));
    }

    #[test]
    fn z_codeaction_pad_to_basic() {
        assert_eq!(z_codeaction_pad_to("hi", 5), "hi   ");
        assert_eq!(z_codeaction_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_codeaction_is_identifier_basic() {
        assert!(z_codeaction_is_identifier("foo_bar"));
        assert!(z_codeaction_is_identifier("abc123"));
        assert!(!z_codeaction_is_identifier(""));
        assert!(!z_codeaction_is_identifier("has space"));
    }

    #[test]
    fn z_codeaction_levenshtein_basic() {
        assert_eq!(z_codeaction_levenshtein("", ""), 0);
        assert_eq!(z_codeaction_levenshtein("abc", "abc"), 0);
        assert_eq!(z_codeaction_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_codeaction_unique_words_basic() {
        let w = z_codeaction_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_codeaction_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_codeaction_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_codeaction_common_prefix_basic() {
        assert_eq!(z_codeaction_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_codeaction_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_codeaction_struct_clear() {
        let mut s = ZCodeactionCodeActionFilter::new();
        s.kind_patterns.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_codeaction_rolling_hash_empty() {
        let h = z_codeaction_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_72_push_and_len() {
        let mut rb = super::XbRingBuffer72::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_72_overwrite() {
        let mut rb = super::XbRingBuffer72::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_72_get_out_of_bounds() {
        let rb = super::XbRingBuffer72::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_72_drain_all() {
        let mut rb = super::XbRingBuffer72::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_72_peek_front_back() {
        let mut rb = super::XbRingBuffer72::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_72_clear() {
        let mut rb = super::XbRingBuffer72::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_72_capacity() {
        let rb = super::XbRingBuffer72::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_72_basic() {
        let h = super::xb_fnv1a_72(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_72(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_72_different_inputs() {
        let h1 = super::xb_fnv1a_72(b"abc");
        let h2 = super::xb_fnv1a_72(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_72_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_72(&data);
        let dec = super::xb_rle_decode_72(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_72_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_72(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_72(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_72_values() {
        assert!((super::xb_clamp_72(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_72(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_72(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_72_values() {
        assert!((super::xb_lerp_72(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_72(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_72(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_72_wrap_around_twice() {
        let mut rb = super::XbRingBuffer72::new(2);
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