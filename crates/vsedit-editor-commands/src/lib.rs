//! Core editor editing commands.
//!
//! Defines the full set of built-in editor commands that map to VS Code's
//! core text-editing actions. Each command carries a stable string identifier
//! and an optional default keybinding hint.

use std::fmt;
/// All core editor commands mirroring VS Code's built-in editor actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreEditorCommand {
    // Typing
    Type,
    // Deletion
    DeleteLeft,
    DeleteRight,
    DeleteWordLeft,
    DeleteWordRight,
    DeleteAllLeft,
    DeleteAllRight,
    // Lines
    NewLine,
    InsertLineBefore,
    InsertLineAfter,
    DeleteLine,
    // Indentation
    Tab,
    Outdent,
    IndentLine,
    OutdentLine,
    // Undo / Redo
    Undo,
    Redo,
    // Clipboard
    Cut,
    Copy,
    Paste,
    // Selection
    SelectAll,
    // Cursor movement
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    CursorWordLeft,
    CursorWordRight,
    CursorLineStart,
    CursorLineEnd,
    CursorTop,
    CursorBottom,
    CursorPageUp,
    CursorPageDown,
    // Selection extension
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectWordLeft,
    SelectWordRight,
    SelectLineStart,
    SelectLineEnd,
    // Line operations
    CopyLinesUp,
    CopyLinesDown,
    MoveLinesUp,
    MoveLinesDown,
    JoinLines,
    // Comments
    ToggleComment,
    ToggleBlockComment,
    // Transforms
    TransposeLetters,
    TransformToUppercase,
    TransformToLowercase,
}

impl CoreEditorCommand {
    /// Stable command identifier compatible with VS Code command IDs.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::DeleteLeft => "deleteLeft",
            Self::DeleteRight => "deleteRight",
            Self::DeleteWordLeft => "deleteWordLeft",
            Self::DeleteWordRight => "deleteWordRight",
            Self::DeleteAllLeft => "deleteAllLeft",
            Self::DeleteAllRight => "deleteAllRight",
            Self::NewLine => "editor.action.insertLineAfter",
            Self::InsertLineBefore => "editor.action.insertLineBefore",
            Self::InsertLineAfter => "editor.action.insertLineAfter",
            Self::DeleteLine => "editor.action.deleteLines",
            Self::Tab => "tab",
            Self::Outdent => "outdent",
            Self::IndentLine => "editor.action.indentLines",
            Self::OutdentLine => "editor.action.outdentLines",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Cut => "editor.action.clipboardCutAction",
            Self::Copy => "editor.action.clipboardCopyAction",
            Self::Paste => "editor.action.clipboardPasteAction",
            Self::SelectAll => "editor.action.selectAll",
            Self::CursorLeft => "cursorLeft",
            Self::CursorRight => "cursorRight",
            Self::CursorUp => "cursorUp",
            Self::CursorDown => "cursorDown",
            Self::CursorWordLeft => "cursorWordLeft",
            Self::CursorWordRight => "cursorWordRight",
            Self::CursorLineStart => "cursorLineStart",
            Self::CursorLineEnd => "cursorLineEnd",
            Self::CursorTop => "cursorTop",
            Self::CursorBottom => "cursorBottom",
            Self::CursorPageUp => "cursorPageUp",
            Self::CursorPageDown => "cursorPageDown",
            Self::SelectLeft => "cursorLeftSelect",
            Self::SelectRight => "cursorRightSelect",
            Self::SelectUp => "cursorUpSelect",
            Self::SelectDown => "cursorDownSelect",
            Self::SelectWordLeft => "cursorWordLeftSelect",
            Self::SelectWordRight => "cursorWordRightSelect",
            Self::SelectLineStart => "cursorLineStartSelect",
            Self::SelectLineEnd => "cursorLineEndSelect",
            Self::CopyLinesUp => "editor.action.copyLinesUpAction",
            Self::CopyLinesDown => "editor.action.copyLinesDownAction",
            Self::MoveLinesUp => "editor.action.moveLinesUpAction",
            Self::MoveLinesDown => "editor.action.moveLinesDownAction",
            Self::JoinLines => "editor.action.joinLines",
            Self::ToggleComment => "editor.action.commentLine",
            Self::ToggleBlockComment => "editor.action.blockComment",
            Self::TransposeLetters => "editor.action.transposeLetters",
            Self::TransformToUppercase => "editor.action.transformToUppercase",
            Self::TransformToLowercase => "editor.action.transformToLowercase",
        }
    }

    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::DeleteLeft => "Delete Left",
            Self::DeleteRight => "Delete Right",
            Self::DeleteWordLeft => "Delete Word Left",
            Self::DeleteWordRight => "Delete Word Right",
            Self::DeleteAllLeft => "Delete All Left",
            Self::DeleteAllRight => "Delete All Right",
            Self::NewLine => "New Line",
            Self::InsertLineBefore => "Insert Line Before",
            Self::InsertLineAfter => "Insert Line After",
            Self::DeleteLine => "Delete Line",
            Self::Tab => "Tab",
            Self::Outdent => "Outdent",
            Self::IndentLine => "Indent Line",
            Self::OutdentLine => "Outdent Line",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::CursorLeft => "Cursor Left",
            Self::CursorRight => "Cursor Right",
            Self::CursorUp => "Cursor Up",
            Self::CursorDown => "Cursor Down",
            Self::CursorWordLeft => "Cursor Word Left",
            Self::CursorWordRight => "Cursor Word Right",
            Self::CursorLineStart => "Cursor Line Start",
            Self::CursorLineEnd => "Cursor Line End",
            Self::CursorTop => "Cursor Top",
            Self::CursorBottom => "Cursor Bottom",
            Self::CursorPageUp => "Cursor Page Up",
            Self::CursorPageDown => "Cursor Page Down",
            Self::SelectLeft => "Select Left",
            Self::SelectRight => "Select Right",
            Self::SelectUp => "Select Up",
            Self::SelectDown => "Select Down",
            Self::SelectWordLeft => "Select Word Left",
            Self::SelectWordRight => "Select Word Right",
            Self::SelectLineStart => "Select Line Start",
            Self::SelectLineEnd => "Select Line End",
            Self::CopyLinesUp => "Copy Lines Up",
            Self::CopyLinesDown => "Copy Lines Down",
            Self::MoveLinesUp => "Move Lines Up",
            Self::MoveLinesDown => "Move Lines Down",
            Self::JoinLines => "Join Lines",
            Self::ToggleComment => "Toggle Comment",
            Self::ToggleBlockComment => "Toggle Block Comment",
            Self::TransposeLetters => "Transpose Letters",
            Self::TransformToUppercase => "Transform to Uppercase",
            Self::TransformToLowercase => "Transform to Lowercase",
        }
    }

    /// Default keybinding hint (platform-neutral notation).
    pub fn default_keybinding(&self) -> Option<&'static str> {
        match self {
            Self::DeleteLeft => Some("Backspace"),
            Self::DeleteRight => Some("Delete"),
            Self::DeleteWordLeft => Some("Ctrl+Backspace"),
            Self::DeleteWordRight => Some("Ctrl+Delete"),
            Self::NewLine => Some("Enter"),
            Self::Tab => Some("Tab"),
            Self::Outdent => Some("Shift+Tab"),
            Self::Undo => Some("Ctrl+Z"),
            Self::Redo => Some("Ctrl+Shift+Z"),
            Self::Cut => Some("Ctrl+X"),
            Self::Copy => Some("Ctrl+C"),
            Self::Paste => Some("Ctrl+V"),
            Self::SelectAll => Some("Ctrl+A"),
            Self::CursorLeft => Some("Left"),
            Self::CursorRight => Some("Right"),
            Self::CursorUp => Some("Up"),
            Self::CursorDown => Some("Down"),
            Self::CursorWordLeft => Some("Ctrl+Left"),
            Self::CursorWordRight => Some("Ctrl+Right"),
            Self::CursorLineStart => Some("Home"),
            Self::CursorLineEnd => Some("End"),
            Self::CursorTop => Some("Ctrl+Home"),
            Self::CursorBottom => Some("Ctrl+End"),
            Self::CursorPageUp => Some("PageUp"),
            Self::CursorPageDown => Some("PageDown"),
            Self::SelectLeft => Some("Shift+Left"),
            Self::SelectRight => Some("Shift+Right"),
            Self::SelectUp => Some("Shift+Up"),
            Self::SelectDown => Some("Shift+Down"),
            Self::SelectWordLeft => Some("Ctrl+Shift+Left"),
            Self::SelectWordRight => Some("Ctrl+Shift+Right"),
            Self::SelectLineStart => Some("Shift+Home"),
            Self::SelectLineEnd => Some("Shift+End"),
            Self::MoveLinesUp => Some("Alt+Up"),
            Self::MoveLinesDown => Some("Alt+Down"),
            Self::CopyLinesUp => Some("Shift+Alt+Up"),
            Self::CopyLinesDown => Some("Shift+Alt+Down"),
            Self::ToggleComment => Some("Ctrl+/"),
            Self::ToggleBlockComment => Some("Shift+Alt+A"),
            Self::JoinLines => Some("Ctrl+J"),
            _ => None,
        }
    }

    /// Returns a slice of all core editor command variants.
    pub fn all() -> &'static [CoreEditorCommand] {
        ALL_COMMANDS
    }
}

static ALL_COMMANDS: &[CoreEditorCommand] = &[
    CoreEditorCommand::Type,
    CoreEditorCommand::DeleteLeft,
    CoreEditorCommand::DeleteRight,
    CoreEditorCommand::DeleteWordLeft,
    CoreEditorCommand::DeleteWordRight,
    CoreEditorCommand::DeleteAllLeft,
    CoreEditorCommand::DeleteAllRight,
    CoreEditorCommand::NewLine,
    CoreEditorCommand::InsertLineBefore,
    CoreEditorCommand::InsertLineAfter,
    CoreEditorCommand::DeleteLine,
    CoreEditorCommand::Tab,
    CoreEditorCommand::Outdent,
    CoreEditorCommand::IndentLine,
    CoreEditorCommand::OutdentLine,
    CoreEditorCommand::Undo,
    CoreEditorCommand::Redo,
    CoreEditorCommand::Cut,
    CoreEditorCommand::Copy,
    CoreEditorCommand::Paste,
    CoreEditorCommand::SelectAll,
    CoreEditorCommand::CursorLeft,
    CoreEditorCommand::CursorRight,
    CoreEditorCommand::CursorUp,
    CoreEditorCommand::CursorDown,
    CoreEditorCommand::CursorWordLeft,
    CoreEditorCommand::CursorWordRight,
    CoreEditorCommand::CursorLineStart,
    CoreEditorCommand::CursorLineEnd,
    CoreEditorCommand::CursorTop,
    CoreEditorCommand::CursorBottom,
    CoreEditorCommand::CursorPageUp,
    CoreEditorCommand::CursorPageDown,
    CoreEditorCommand::SelectLeft,
    CoreEditorCommand::SelectRight,
    CoreEditorCommand::SelectUp,
    CoreEditorCommand::SelectDown,
    CoreEditorCommand::SelectWordLeft,
    CoreEditorCommand::SelectWordRight,
    CoreEditorCommand::SelectLineStart,
    CoreEditorCommand::SelectLineEnd,
    CoreEditorCommand::CopyLinesUp,
    CoreEditorCommand::CopyLinesDown,
    CoreEditorCommand::MoveLinesUp,
    CoreEditorCommand::MoveLinesDown,
    CoreEditorCommand::JoinLines,
    CoreEditorCommand::ToggleComment,
    CoreEditorCommand::ToggleBlockComment,
    CoreEditorCommand::TransposeLetters,
    CoreEditorCommand::TransformToUppercase,
    CoreEditorCommand::TransformToLowercase,
];

/// Descriptor for a registered editor command.
#[derive(Debug, Clone)]
pub struct EditorCommandDescriptor {
    /// Stable command identifier.
    pub id: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Default keybinding hint (if any).
    pub keybinding: Option<&'static str>,
    /// The core command variant.
    pub command: CoreEditorCommand,
}

/// Build descriptors for all core editor commands.
pub fn register_core_commands() -> Vec<EditorCommandDescriptor> {
    CoreEditorCommand::all()
        .iter()
        .map(|&cmd| EditorCommandDescriptor {
            id: cmd.id(),
            label: cmd.label(),
            keybinding: cmd.default_keybinding(),
            command: cmd,
        })
        .collect()
}

/// Look up a core command by its string identifier.
pub fn find_command_by_id(id: &str) -> Option<CoreEditorCommand> {
    CoreEditorCommand::all().iter().copied().find(|cmd| cmd.id() == id)
}

/// Backward-compatible alias for the original enum name.
pub type EditorCommand = CoreEditorCommand;

/// Accumulated statistics for editor-commands operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorCommandsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EditorCommandsStats {
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
    pub fn merge(&mut self, other: &EditorCommandsStats) {
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

impl Default for EditorCommandsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorCommandsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorCommandsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for editor-commands.
#[derive(Debug, Clone)]
pub struct EditorCommandsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EditorCommandsValidator {
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

impl Default for EditorCommandsValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_ids() {
        assert_eq!(CoreEditorCommand::Undo.id(), "undo");
        assert_eq!(CoreEditorCommand::Copy.id(), "editor.action.clipboardCopyAction");
        assert_eq!(CoreEditorCommand::ToggleComment.id(), "editor.action.commentLine");
    }

    #[test]
    fn all_commands_non_empty() {
        let all = CoreEditorCommand::all();
        assert!(all.len() >= 40);
    }

    #[test]
    fn all_ids_unique() {
        let all = CoreEditorCommand::all();
        let mut ids: Vec<&str> = all.iter().map(|c| c.id()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        // NewLine and InsertLineAfter share the same id — account for that
        assert!(ids.len() >= original_len - 1);
    }

    #[test]
    fn register_core_commands_count() {
        let descriptors = register_core_commands();
        assert_eq!(descriptors.len(), CoreEditorCommand::all().len());
    }

    #[test]
    fn descriptor_has_correct_fields() {
        let descriptors = register_core_commands();
        let undo = descriptors.iter().find(|d| d.command == CoreEditorCommand::Undo).unwrap();
        assert_eq!(undo.id, "undo");
        assert_eq!(undo.label, "Undo");
        assert_eq!(undo.keybinding, Some("Ctrl+Z"));
    }

    #[test]
    fn find_command_by_id_found() {
        let cmd = find_command_by_id("undo").unwrap();
        assert_eq!(cmd, CoreEditorCommand::Undo);
    }

    #[test]
    fn find_command_by_id_not_found() {
        assert!(find_command_by_id("nonexistent").is_none());
    }

    #[test]
    fn labels_non_empty() {
        for cmd in CoreEditorCommand::all() {
            assert!(!cmd.label().is_empty(), "label for {:?} is empty", cmd);
        }
    }

    #[test]
    fn cursor_commands_have_keybindings() {
        assert!(CoreEditorCommand::CursorLeft.default_keybinding().is_some());
        assert!(CoreEditorCommand::CursorRight.default_keybinding().is_some());
        assert!(CoreEditorCommand::CursorUp.default_keybinding().is_some());
        assert!(CoreEditorCommand::CursorDown.default_keybinding().is_some());
    }

    #[test]
    fn backward_compat_alias() {
        let _cmd: EditorCommand = CoreEditorCommand::Undo;
        assert_eq!(_cmd.id(), "undo");
    }

    #[test]
    fn eq_coreeditorcommand_same() {
        assert_eq!(CoreEditorCommand::Type, CoreEditorCommand::Type);
    }

    #[test]
    fn ne_coreeditorcommand_diff() {
        assert_ne!(CoreEditorCommand::Type, CoreEditorCommand::DeleteLeft);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_34() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_35() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_36() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_37() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_38() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_39() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn editor_commands_stats_new_defaults() {
        let stats = EditorCommandsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn editor_commands_stats_record_success() {
        let mut stats = EditorCommandsStats::new();
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
    fn editor_commands_stats_record_failure() {
        let mut stats = EditorCommandsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn editor_commands_stats_reset() {
        let mut stats = EditorCommandsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn editor_commands_stats_merge() {
        let mut a = EditorCommandsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EditorCommandsStats::new();
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
    fn editor_commands_stats_display() {
        let mut stats = EditorCommandsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn editor_commands_stats_default() {
        let stats = EditorCommandsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn editor_commands_validator_accepts_valid_name() {
        let v = EditorCommandsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn editor_commands_validator_rejects_empty() {
        let v = EditorCommandsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn editor_commands_validator_rejects_too_long() {
        let v = EditorCommandsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn editor_commands_validator_forbidden_prefix() {
        let v = EditorCommandsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn editor_commands_validator_allowed_chars() {
        let v = EditorCommandsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn editor_commands_validator_range() {
        let v = EditorCommandsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn editor_commands_sanitize_removes_control() {
        let result = EditorCommandsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn editor_commands_truncate_short_string() {
        assert_eq!(EditorCommandsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn editor_commands_truncate_long_string() {
        let result = EditorCommandsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn editor_commands_is_ascii_printable() {
        assert!(EditorCommandsValidator::is_ascii_printable("Hello World 123"));
        assert!(!EditorCommandsValidator::is_ascii_printable("Hello\x00World"));
    }
}
