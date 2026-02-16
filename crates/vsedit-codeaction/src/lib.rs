//! Quick fix and refactoring.

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
}

/// A text edit within a code action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEdit {
    pub uri: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
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
            is_preferred: false,
            disabled_reason: None,
        }
    }

    pub fn with_edit(mut self, edit: WorkspaceEdit) -> Self {
        self.edit = Some(edit);
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
// Apply edit
// ---------------------------------------------------------------------------

/// Apply a single `WorkspaceEdit` to `text`, treating it as a flat string
/// addressed by line/column offsets. Only edits on the same content are useful.
pub fn apply_edit(text: &str, edit: &WorkspaceEdit) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = String::new();

    for (i, line) in lines.iter().enumerate() {
        let line_idx = i as u32;
        if line_idx < edit.start_line || line_idx > edit.end_line {
            result.push_str(line);
            result.push('\n');
        } else if line_idx == edit.start_line && line_idx == edit.end_line {
            let start = edit.start_col as usize;
            let end = edit.end_col as usize;
            result.push_str(&line[..start.min(line.len())]);
            result.push_str(&edit.new_text);
            if end < line.len() {
                result.push_str(&line[end..]);
            }
            result.push('\n');
        } else if line_idx == edit.start_line {
            let start = edit.start_col as usize;
            result.push_str(&line[..start.min(line.len())]);
            result.push_str(&edit.new_text);
        } else if line_idx == edit.end_line {
            let end = edit.end_col as usize;
            if end < line.len() {
                result.push_str(&line[end..]);
            }
            result.push('\n');
        }
        // lines strictly between start and end are dropped
    }

    // Remove trailing newline if original didn't end with one
    if !text.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_action_creation() {
        let action = CodeAction::new("Fix typo", CodeActionKind::QuickFix)
            .preferred()
            .with_edit(WorkspaceEdit {
                uri: "file:///test.rs".into(),
                start_line: 1, start_col: 0, end_line: 1, end_col: 5,
                new_text: "fixed".into(),
            });
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
    fn apply_edit_single_line() {
        let text = "fn main() {\n    old_call();\n}";
        let edit = WorkspaceEdit {
            uri: "file:///test.rs".into(),
            start_line: 1,
            start_col: 4,
            end_line: 1,
            end_col: 14,
            new_text: "new_call()".into(),
        };
        let result = apply_edit(text, &edit);
        assert!(result.contains("new_call()"));
        assert!(!result.contains("old_call()"));
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
            .with_edit(WorkspaceEdit {
                uri: "f".into(),
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                new_text: "x".into(),
            });
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
}
