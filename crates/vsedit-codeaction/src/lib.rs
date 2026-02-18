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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 15
// ---------------------------------------------------------------------------

/// Generic object pool `Xc15Pool<T>`.
pub struct Xc15Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc15Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc15PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc15Pool<T> {
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
    pub fn stats(&self) -> Xc15PoolStats {
        Xc15PoolStats {
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

impl<T> Default for Xc15Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc15Scheduler`.
pub struct Xc15Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc15Scheduler {
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

impl Default for Xc15Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_15 hash for the given byte slice.
pub fn xc_15_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_15 convention.
pub fn xc_15_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe85 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe85Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe85PipelineError {
    pub stage: Xe85Stage,
    pub message: String,
}

impl std::fmt::Display for Xe85PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe85Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe85Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError>>>,
    stage_names: Vec<Xe85Stage>,
}

impl Xe85Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe85Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe85Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe85Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe85Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
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

    pub fn compose(mut self, other: Xe85Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe85CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe85CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe85Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe85CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe85CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe85Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe85CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_85_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe85CacheEntry {
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

    fn xe_85_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe85CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_85_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
    Ok(data)
}

pub fn xe_85_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_85_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_85_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_85_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe85PipelineError> {
    Err(Xe85PipelineError {
        stage: Xe85Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_83: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg83Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg83Graph {
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

impl Default for Xg83Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_83: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg83Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg83Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg83Heap<T>) {
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

impl<T: Ord> Default for Xg83Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 14).
pub struct Xh14SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh14SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 56 as u64,
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

/// A compact bit set supporting boolean operations (variant 14).
pub struct Xh14BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh14BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 14).
pub struct Xi14Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi14Deque<T> {
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
pub struct Xi14Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi14Interval {
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

/// A simple interval tree (variant 14).
pub struct Xi14IntervalTree {
    xi_intervals: Vec<Xi14Interval>,
}

impl Xi14IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi14Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi14Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi14Interval) -> Vec<&Xi14Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi14Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi14Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi14Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi14Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi14Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi14Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 14) ---

/// Disjoint set / union-find for crate 14.
pub struct Xj14UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj14UnionFind {
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

const XJ14_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 14.
pub struct Xj14BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj14BTreeNode<K, V>>>,
    len: usize,
}

struct Xj14BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj14BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj14BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ14_BTREE_ORDER - 1
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
        let mid = XJ14_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj14BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj14BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj14BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj14BTreeNode::xj_new_leaf();
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


// --- xk_14 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk14SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk14SegmentTree {
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
pub struct Xk14DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk14DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_14).
#[derive(Debug, Clone)]
pub struct Xl14Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl14Rope {
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

/// Suffix array for efficient string searching (xl_14).
#[derive(Debug, Clone)]
pub struct Xl14SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl14SuffixArray {
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
pub struct Xm14MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm14MatrixSparse {
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
pub struct Xm14Tokenizer {
    text: String,
}

impl Xm14Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 14.
pub struct Xn14Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn14Fenwick {
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

// ----- AVL tree map — crate 14 -----

#[derive(Debug, Clone)]
struct Xn14AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn14AvlNode<K, V>>>,
    right: Option<Box<Xn14AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 14.
#[derive(Debug, Clone)]
pub struct Xn14AVL<K, V> {
    root: Option<Box<Xn14AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn14AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn14AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn14AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn14AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn14AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn14AvlNode<K, V>>) -> Box<Xn14AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn14AvlNode<K, V>>) -> Box<Xn14AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn14AvlNode<K, V>>) -> Box<Xn14AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn14AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn14AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn14AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn14AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn14AvlNode<K, V>>) -> &Xn14AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn14AvlNode<K, V>>) -> (Box<Xn14AvlNode<K, V>>, Option<Box<Xn14AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn14AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn14AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn14AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn14AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn14AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn14AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn14AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo14RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo14Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo14RBNode<K, V> {
    key: K,
    value: V,
    color: Xo14Color,
    left: Option<Box<Xo14RBNode<K, V>>>,
    right: Option<Box<Xo14RBNode<K, V>>>,
}

/// A red-black tree map for crate 14.
#[derive(Debug, Clone)]
pub struct Xo14RedBlack<K, V> {
    root: Option<Box<Xo14RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo14RedBlack<K, V> {
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
            r.color = Xo14Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo14RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo14RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo14RBNode {
                    key, value, color: Xo14Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo14RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo14Color::Red)
    }

    fn xo_balance(mut h: Box<Xo14RBNode<K, V>>) -> Box<Xo14RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo14Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo14RBNode<K, V>>) -> Box<Xo14RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo14Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo14RBNode<K, V>>) -> Box<Xo14RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo14Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo14RBNode<K, V>>) {
        h.color = Xo14Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo14Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo14Color::Black; }
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
            r.color = Xo14Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo14RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo14RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo14RBNode<K, V>) -> (K, V, Option<Box<Xo14RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo14RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo14Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo14RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo14ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 14.
#[derive(Debug, Clone)]
pub struct Xo14ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo14ConsistentHash {
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
            let vkey = format!("{}#xo14#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo14#{}", node, i);
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


    // ---- xc_ pool / scheduler tests – block 15 ----

    #[test]
    fn xc_15_pool_new_empty() {
        let pool: super::Xc15Pool<i32> = super::Xc15Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_15_pool_release_acquire() {
        let mut pool = super::Xc15Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_15_pool_acquire_empty() {
        let mut pool: super::Xc15Pool<i32> = super::Xc15Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_15_pool_full() {
        let mut pool = super::Xc15Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_15_pool_drain() {
        let mut pool = super::Xc15Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_15_pool_stats() {
        let mut pool = super::Xc15Pool::new(8);
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
    fn xc_15_pool_clear() {
        let mut pool = super::Xc15Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_15_pool_shrink() {
        let mut pool = super::Xc15Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_15_pool_default() {
        let pool: super::Xc15Pool<String> = super::Xc15Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_15_pool_extend() {
        let mut pool = super::Xc15Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_15_pool_retain() {
        let mut pool = super::Xc15Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_15_scheduler_round_robin() {
        let mut sched = super::Xc15Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_15_scheduler_empty() {
        let mut sched = super::Xc15Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_15_scheduler_reset() {
        let mut sched = super::Xc15Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_15_scheduler_add_remove() {
        let mut sched = super::Xc15Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_15_scheduler_targets() {
        let sched = super::Xc15Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_15_hash_empty() {
        assert_eq!(super::xc_15_hash(b""), 5381);
    }

    #[test]
    fn xc_15_hash_data() {
        let h = super::xc_15_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_15_hash(b"hello"), h);
    }

    #[test]
    fn xc_15_reverse_str() {
        assert_eq!(super::xc_15_reverse("abc"), "cba");
        assert_eq!(super::xc_15_reverse(""), "");
    }


    #[test]
    fn xe_85_pipeline_empty() {
        let p = super::Xe85Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_85_pipeline_parse_stage() {
        let p = super::Xe85Pipeline::new()
            .add_parse(super::xe_85_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_85_pipeline_transform_double() {
        let p = super::Xe85Pipeline::new()
            .add_transform(super::xe_85_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_85_pipeline_validate_reverse() {
        let p = super::Xe85Pipeline::new()
            .add_validate(super::xe_85_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_85_pipeline_emit_filter() {
        let p = super::Xe85Pipeline::new()
            .add_emit(super::xe_85_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_85_pipeline_multi_stage() {
        let p = super::Xe85Pipeline::new()
            .add_parse(super::xe_85_pipeline_identity)
            .add_transform(super::xe_85_pipeline_double)
            .add_validate(super::xe_85_pipeline_reverse)
            .add_emit(super::xe_85_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_85_pipeline_error_propagation() {
        let p = super::Xe85Pipeline::new()
            .add_parse(super::xe_85_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe85Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_85_pipeline_compose() {
        let p1 = super::Xe85Pipeline::new()
            .add_parse(super::xe_85_pipeline_identity);
        let p2 = super::Xe85Pipeline::new()
            .add_transform(super::xe_85_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_85_pipeline_error_display() {
        let e = super::Xe85PipelineError {
            stage: super::Xe85Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_85_cache_put_get() {
        let mut c = super::Xe85Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_85_cache_miss() {
        let mut c: super::Xe85Cache<&str, i32> = super::Xe85Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_85_cache_ttl_expiry() {
        let mut c = super::Xe85Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_85_cache_evict() {
        let mut c = super::Xe85Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_85_cache_capacity() {
        let mut c = super::Xe85Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_85_cache_stats() {
        let mut c = super::Xe85Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_85_cache_clear() {
        let mut c = super::Xe85Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_83 graph tests ------------------------------------------------

    #[test]
    fn xg_83_graph_empty() {
        let g = super::Xg83Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_83_graph_add_node() {
        let mut g = super::Xg83Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_83_graph_add_edge() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_83_graph_neighbors() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_83_graph_has_path() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_83_graph_self_path() {
        let g = super::Xg83Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_83_graph_topo_sort() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_83_graph_cycle_detect_false() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_83_graph_cycle_detect_true() {
        let mut g = super::Xg83Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_83 heap tests -------------------------------------------------

    #[test]
    fn xg_83_heap_empty() {
        let h: super::Xg83Heap<i32> = super::Xg83Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_83_heap_push_pop() {
        let mut h = super::Xg83Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_83_heap_peek() {
        let mut h = super::Xg83Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_83_heap_drain_sorted() {
        let mut h = super::Xg83Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_83_heap_merge() {
        let mut a = super::Xg83Heap::new();
        let mut b = super::Xg83Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_83_heap_default() {
        let h: super::Xg83Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_83_graph_default() {
        let g: super::Xg83Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh14_skip_insert_contains() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh14_skip_remove() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh14_skip_len() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh14_skip_range_query() {
        let mut sl = super::Xh14SkipList::xh_new(4);
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
    fn xh14_skip_floor_ceiling() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh14_skip_rank() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh14_skip_empty() {
        let sl = super::Xh14SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh14_skip_duplicates() {
        let mut sl = super::Xh14SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh14_bitset_set_test() {
        let mut bs = super::Xh14BitSet::xh_new(256);
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
    fn xh14_bitset_clear_count() {
        let mut bs = super::Xh14BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh14_bitset_and_or_xor() {
        let mut a = super::Xh14BitSet::xh_new(128);
        let mut b = super::Xh14BitSet::xh_new(128);
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
    fn xh14_bitset_iter_ones() {
        let mut bs = super::Xh14BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh14_bitset_first_last() {
        let mut bs = super::Xh14BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh14_bitset_empty() {
        let bs = super::Xh14BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi14_deque_push_pop_back() {
        let mut dq = super::Xi14Deque::xi_new(4);
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
    fn xi14_deque_push_pop_front() {
        let mut dq = super::Xi14Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi14_deque_mixed_ops() {
        let mut dq = super::Xi14Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi14_deque_get_and_split() {
        let mut dq = super::Xi14Deque::xi_new(8);
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
    fn xi14_deque_rotate_left() {
        let mut dq = super::Xi14Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi14_deque_rotate_right() {
        let mut dq = super::Xi14Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi14_deque_grow() {
        let mut dq = super::Xi14Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi14_deque_empty() {
        let dq = super::Xi14Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi14_interval_tree_insert_query() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi14Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi14Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi14_interval_tree_overlap() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi14Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi14Interval::xi_new(12, 20));
        let q = super::Xi14Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi14_interval_tree_remove() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi14Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi14_interval_tree_gaps() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi14Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi14Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi14Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi14Interval::xi_new(8, 10));
    }

    #[test]
    fn xi14_interval_tree_merge() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi14Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi14Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi14Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi14Interval::xi_new(10, 15));
    }

    #[test]
    fn xi14_interval_tree_all() {
        let mut tree = super::Xi14IntervalTree::xi_new();
        tree.xi_insert(super::Xi14Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi14Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi14_interval_tree_empty() {
        let tree = super::Xi14IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi14_interval_tree_contains_point() {
        let iv = super::Xi14Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 14) ---

    #[test]
    fn xj_14_uf_make_and_find() {
        let mut uf = super::Xj14UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_14_uf_union_connected() {
        let mut uf = super::Xj14UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_14_uf_component_count() {
        let mut uf = super::Xj14UnionFind::xj_new();
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
    fn xj_14_uf_component_size() {
        let mut uf = super::Xj14UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_14_uf_largest_component() {
        let mut uf = super::Xj14UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_14_uf_many_elements() {
        let mut uf = super::Xj14UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_14_uf_separate_components() {
        let mut uf = super::Xj14UnionFind::xj_new();
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
    fn xj_14_uf_path_compression() {
        let mut uf = super::Xj14UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_14_bt_insert_get() {
        let mut bt = super::Xj14BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_14_bt_contains_len() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_14_bt_replace() {
        let mut bt = super::Xj14BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_14_bt_remove() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_14_bt_keys_values() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_14_bt_range() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_14_bt_min_max() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_14_bt_many_inserts() {
        let mut bt = super::Xj14BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_14 segment tree tests ---

    #[test]
    fn xk_14_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_14_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk14SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_14_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_14_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_14_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_14_st_single_element() {
        let data = vec![42];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_14_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk14SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_14_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk14SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_14 disjoint intervals tests ---

    #[test]
    fn xk_14_di_add_and_count() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_14_di_merge_overlap() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_14_di_contains() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_14_di_remove() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_14_di_covered_length() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_14_di_gaps() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_14_di_merge_adjacent() {
        let mut di = super::Xk14DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_14_di_empty() {
        let di = super::Xk14DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_14_rope_new_empty() {
        let rope = super::Xl14Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_14_rope_from_str() {
        let rope = super::Xl14Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_14_rope_insert_at() {
        let mut rope = super::Xl14Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_14_rope_delete_range() {
        let mut rope = super::Xl14Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_14_rope_char_at() {
        let rope = super::Xl14Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_14_rope_split_concat() {
        let rope = super::Xl14Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_14_rope_line_count() {
        let rope = super::Xl14Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_14_rope_line_at() {
        let rope = super::Xl14Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_14_sa_build_and_search() {
        let sa = super::Xl14SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_14_sa_count() {
        let sa = super::Xl14SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_14_sa_longest_repeated() {
        let sa = super::Xl14SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_14_sa_all_positions() {
        let sa = super::Xl14SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_14_sa_len() {
        let sa = super::Xl14SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_14_sa_empty() {
        let sa = super::Xl14SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_14_rope_slice() {
        let rope = super::Xl14Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_14_sa_search_start() {
        let sa = super::Xl14SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_14_sparse_set_get() {
        let mut m = super::Xm14MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_14_sparse_row_col() {
        let mut m = super::Xm14MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_14_sparse_transpose() {
        let mut m = super::Xm14MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_14_sparse_multiply_vec() {
        let mut m = super::Xm14MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_14_sparse_nnz_density() {
        let mut m = super::Xm14MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_14_sparse_clear() {
        let mut m = super::Xm14MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_14_sparse_overwrite_zero() {
        let mut m = super::Xm14MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_14_tokenizer_basic() {
        let t = super::Xm14Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_14_tokenizer_count() {
        let t = super::Xm14Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_14_tokenizer_unique() {
        let t = super::Xm14Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_14_tokenizer_frequency() {
        let t = super::Xm14Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_14_tokenizer_delimiter() {
        let t = super::Xm14Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_14_tokenizer_whitespace() {
        let t = super::Xm14Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_14_tokenizer_empty() {
        let t = super::Xm14Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 14 ----

    #[test]
    fn xn_14_fenwick_prefix_sum() {
        let mut ft = super::Xn14Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_14_fenwick_range_sum() {
        let mut ft = super::Xn14Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_14_fenwick_point_query() {
        let mut ft = super::Xn14Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_14_fenwick_len() {
        let ft = super::Xn14Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_14_fenwick_multiple_updates() {
        let mut ft = super::Xn14Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_14_fenwick_single_element() {
        let mut ft = super::Xn14Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_14_fenwick_find_kth() {
        let mut ft = super::Xn14Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_14_fenwick_negative_delta() {
        let mut ft = super::Xn14Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 14 ----

    #[test]
    fn xn_14_avl_insert_get() {
        let mut m = super::Xn14AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_14_avl_remove() {
        let mut m = super::Xn14AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_14_avl_in_order() {
        let mut m = super::Xn14AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_14_avl_min_max() {
        let mut m = super::Xn14AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_14_avl_floor_ceiling() {
        let mut m = super::Xn14AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_14_avl_height_balanced() {
        let mut m = super::Xn14AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_14_avl_overwrite() {
        let mut m = super::Xn14AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_14_avl_empty() {
        let m: super::Xn14AVL<i32, i32> = super::Xn14AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo14RedBlack tests ---

    #[test]
    fn xo_14_rb_insert_and_get() {
        let mut tree = super::Xo14RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_14_rb_len_and_empty() {
        let mut tree = super::Xo14RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_14_rb_min_max() {
        let mut tree = super::Xo14RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_14_rb_contains() {
        let mut tree = super::Xo14RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_14_rb_remove() {
        let mut tree = super::Xo14RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_14_rb_in_order() {
        let mut tree = super::Xo14RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_14_rb_black_height() {
        let mut tree = super::Xo14RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_14_rb_overwrite() {
        let mut tree = super::Xo14RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo14ConsistentHash tests ---

    #[test]
    fn xo_14_ch_add_and_count() {
        let mut ring = super::Xo14ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_14_ch_remove_node() {
        let mut ring = super::Xo14ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_14_ch_get_node() {
        let mut ring = super::Xo14ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_14_ch_empty_ring() {
        let ring = super::Xo14ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_14_ch_distribution() {
        let mut ring = super::Xo14ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_14_ch_rebalance() {
        let mut ring = super::Xo14ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_14_ch_virtual_nodes() {
        let mut ring = super::Xo14ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_14_ch_consistent_lookup() {
        let mut ring = super::Xo14ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}