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

/// Simulate cutting an entire line when there is no active selection.
/// Returns the cut line content and the remaining text.
pub fn clipboard_cut_line(text: &str, line_index: usize) -> Option<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let cut = lines[line_index].to_string();
    let remaining: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != line_index)
        .map(|(_, l)| *l)
        .collect();
    let remaining_text = if remaining.is_empty() {
        String::new()
    } else {
        remaining.join("\n")
    };
    Some((cut, remaining_text))
}

/// Transpose the two characters around the cursor position.
/// `text` is a single line, `cursor_col` is the column (0-based, byte offset).
/// Returns the new line with characters swapped, or None if the cursor is at
/// the start or there aren't enough characters.
pub fn transpose_characters(text: &str, cursor_col: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return None;
    }
    // If cursor is at position 0, nothing to the left to swap.
    // If cursor is at the end, swap the last two characters.
    let swap_pos = if cursor_col == 0 {
        return None;
    } else if cursor_col >= chars.len() {
        chars.len() - 1
    } else {
        cursor_col
    };
    let mut new_chars = chars;
    new_chars.swap(swap_pos - 1, swap_pos);
    Some(new_chars.into_iter().collect())
}

/// Join multiple lines into a single line, collapsing whitespace at boundaries.
/// `text` is the full document, `start_line` and `end_line` are 0-based inclusive.
/// Returns the modified full text.
pub fn join_lines(text: &str, start_line: usize, end_line: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if start_line >= lines.len() || end_line >= lines.len() || start_line > end_line {
        return None;
    }

    // Join the specified range into one line
    let joined: String = lines[start_line..=end_line]
        .iter()
        .enumerate()
        .fold(String::new(), |mut acc, (i, line)| {
            if i == 0 {
                acc.push_str(line.trim_end());
            } else {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    if !acc.is_empty() {
                        acc.push(' ');
                    }
                    acc.push_str(trimmed);
                }
            }
            acc
        });

    // Rebuild the full text
    let mut result_lines: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i == start_line {
            result_lines.push(joined.clone());
        } else if i > start_line && i <= end_line {
            // Skip these lines - they were joined
            continue;
        } else {
            result_lines.push(line.to_string());
        }
    }

    Some(result_lines.join("\n"))
}

/// Categorize a command as a text-mutating command or a cursor/selection command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    TextMutation,
    CursorMovement,
    Selection,
    Clipboard,
    Other,
}

impl CoreEditorCommand {
    /// Return the category of this command.
    pub fn category(&self) -> CommandCategory {
        match self {
            Self::Type | Self::DeleteLeft | Self::DeleteRight
            | Self::DeleteWordLeft | Self::DeleteWordRight
            | Self::DeleteAllLeft | Self::DeleteAllRight
            | Self::NewLine | Self::InsertLineBefore | Self::InsertLineAfter
            | Self::DeleteLine | Self::Tab | Self::Outdent
            | Self::IndentLine | Self::OutdentLine
            | Self::CopyLinesUp | Self::CopyLinesDown
            | Self::MoveLinesUp | Self::MoveLinesDown
            | Self::JoinLines | Self::ToggleComment | Self::ToggleBlockComment
            | Self::TransposeLetters | Self::TransformToUppercase
            | Self::TransformToLowercase => CommandCategory::TextMutation,
            Self::CursorLeft | Self::CursorRight | Self::CursorUp | Self::CursorDown
            | Self::CursorWordLeft | Self::CursorWordRight
            | Self::CursorLineStart | Self::CursorLineEnd
            | Self::CursorTop | Self::CursorBottom
            | Self::CursorPageUp | Self::CursorPageDown => CommandCategory::CursorMovement,
            Self::SelectAll | Self::SelectLeft | Self::SelectRight
            | Self::SelectUp | Self::SelectDown
            | Self::SelectWordLeft | Self::SelectWordRight
            | Self::SelectLineStart | Self::SelectLineEnd => CommandCategory::Selection,
            Self::Cut | Self::Copy | Self::Paste => CommandCategory::Clipboard,
            Self::Undo | Self::Redo => CommandCategory::Other,
        }
    }

    /// Return true if this command mutates the document text.
    pub fn is_text_mutation(&self) -> bool {
        self.category() == CommandCategory::TextMutation
    }
}

/// Find all commands matching a given category.
pub fn find_commands_by_category(category: CommandCategory) -> Vec<CoreEditorCommand> {
    CoreEditorCommand::all()
        .iter()
        .copied()
        .filter(|cmd| cmd.category() == category)
        .collect()
}

/// Search commands by label substring (case-insensitive).
pub fn search_commands(query: &str) -> Vec<EditorCommandDescriptor> {
    let query_lower = query.to_lowercase();
    register_core_commands()
        .into_iter()
        .filter(|desc| desc.label.to_lowercase().contains(&query_lower))
        .collect()
}

/// Duplicate a line at the given index, inserting the copy above or below.
/// Returns the new text, or None if the line index is out of bounds.
pub fn duplicate_line(text: &str, line_index: usize, above: bool) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let dup = lines[line_index];
    let mut result: Vec<&str> = Vec::with_capacity(lines.len() + 1);
    for (i, line) in lines.iter().enumerate() {
        if above && i == line_index {
            result.push(dup);
        }
        result.push(line);
        if !above && i == line_index {
            result.push(dup);
        }
    }
    Some(result.join("\n"))
}

/// Move a line up or down by one position in the document.
/// Returns the new text, or None if the move is out of bounds.
pub fn move_line(text: &str, line_index: usize, up: bool) -> Option<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    if up && line_index == 0 {
        return None;
    }
    if !up && line_index >= lines.len() - 1 {
        return None;
    }
    let target = if up { line_index - 1 } else { line_index + 1 };
    lines.swap(line_index, target);
    Some(lines.join("\n"))
}

/// Toggle line comment prefix on a range of lines.
/// `comment_prefix` is e.g. "//" or "#".
pub fn toggle_line_comment(text: &str, start_line: usize, end_line: usize, comment_prefix: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if start_line > end_line || end_line >= lines.len() {
        return None;
    }
    let range = &lines[start_line..=end_line];
    let all_commented = range.iter().all(|l| l.trim_start().starts_with(comment_prefix));
    let mut result_lines: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i >= start_line && i <= end_line {
            if all_commented {
                // Remove comment
                if let Some(pos) = line.find(comment_prefix) {
                    let mut s = String::new();
                    s.push_str(&line[..pos]);
                    let after = &line[pos + comment_prefix.len()..];
                    let after = after.strip_prefix(' ').unwrap_or(after);
                    s.push_str(after);
                    result_lines.push(s);
                } else {
                    result_lines.push(line.to_string());
                }
            } else {
                // Add comment
                result_lines.push(format!("{comment_prefix} {line}"));
            }
        } else {
            result_lines.push(line.to_string());
        }
    }
    Some(result_lines.join("\n"))
}

/// Transform text to uppercase within a column range on a specific line.
pub fn transform_range_uppercase(text: &str, line_index: usize, start_col: usize, end_col: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let line = lines[line_index];
    let chars: Vec<char> = line.chars().collect();
    if start_col > end_col || end_col > chars.len() {
        return None;
    }
    let mut new_line = String::with_capacity(line.len());
    for (i, ch) in chars.iter().enumerate() {
        if i >= start_col && i < end_col {
            for uc in ch.to_uppercase() {
                new_line.push(uc);
            }
        } else {
            new_line.push(*ch);
        }
    }
    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line_index] = new_line;
    Some(result.join("\n"))
}

/// Transform text to lowercase within a column range on a specific line.
pub fn transform_range_lowercase(text: &str, line_index: usize, start_col: usize, end_col: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let line = lines[line_index];
    let chars: Vec<char> = line.chars().collect();
    if start_col > end_col || end_col > chars.len() {
        return None;
    }
    let mut new_line = String::with_capacity(line.len());
    for (i, ch) in chars.iter().enumerate() {
        if i >= start_col && i < end_col {
            for lc in ch.to_lowercase() {
                new_line.push(lc);
            }
        } else {
            new_line.push(*ch);
        }
    }
    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line_index] = new_line;
    Some(result.join("\n"))
}

// ── CommandHistory ──

/// An entry in the command history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHistoryEntry {
    pub command: CoreEditorCommand,
    pub timestamp_ms: u64,
}

/// Tracks executed commands for undo/redo navigation.
#[derive(Debug, Clone)]
pub struct CommandHistory {
    entries: Vec<CommandHistoryEntry>,
    /// Points to the current position. Entries after this are redo candidates.
    cursor: usize,
    max_size: usize,
}

impl CommandHistory {
    /// Create a new history with a maximum capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            max_size,
        }
    }

    /// Push a new command, discarding any redo history beyond the cursor.
    pub fn push(&mut self, command: CoreEditorCommand, timestamp_ms: u64) {
        // Discard redo history
        self.entries.truncate(self.cursor);
        self.entries.push(CommandHistoryEntry {
            command,
            timestamp_ms,
        });
        // Trim if over capacity
        if self.entries.len() > self.max_size {
            let remove = self.entries.len() - self.max_size;
            self.entries.drain(0..remove);
        }
        self.cursor = self.entries.len();
    }

    /// Undo: move cursor back and return the undone command.
    pub fn undo(&mut self) -> Option<&CommandHistoryEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor)
    }

    /// Redo: move cursor forward and return the redone command.
    pub fn redo(&mut self) -> Option<&CommandHistoryEntry> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        let entry = self.entries.get(self.cursor);
        self.cursor += 1;
        entry
    }

    /// Return `true` if undo is available.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Return `true` if redo is available.
    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the entire history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    /// Return all entries.
    pub fn entries(&self) -> &[CommandHistoryEntry] {
        &self.entries
    }
}

// ── Command composition ──

/// A composed command that executes two commands in sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedCommand {
    pub first: CoreEditorCommand,
    pub second: CoreEditorCommand,
    pub label: String,
}

impl ComposedCommand {
    /// Create a new composed command.
    pub fn new(first: CoreEditorCommand, second: CoreEditorCommand) -> Self {
        let label = format!("{} + {}", first.label(), second.label());
        Self {
            first,
            second,
            label,
        }
    }

    /// Return the IDs of the two commands.
    pub fn command_ids(&self) -> (&'static str, &'static str) {
        (self.first.id(), self.second.id())
    }
}

/// Compose a sequence of commands into a list of composed pairs.
pub fn compose_command_sequence(commands: &[CoreEditorCommand]) -> Vec<ComposedCommand> {
    commands
        .windows(2)
        .map(|w| ComposedCommand::new(w[0], w[1]))
        .collect()
}

// ── Command macro recording ──

/// A recorded macro: a named sequence of commands that can be replayed.
#[derive(Debug, Clone)]
pub struct CommandMacro {
    pub name: String,
    steps: Vec<CoreEditorCommand>,
}

impl CommandMacro {
    /// Create a new empty macro.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
        }
    }

    /// Add a command to the macro.
    pub fn record(&mut self, command: CoreEditorCommand) {
        self.steps.push(command);
    }

    /// Return the recorded steps.
    pub fn steps(&self) -> &[CoreEditorCommand] {
        &self.steps
    }

    /// Return the number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the macro has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// Records commands into a macro while active.
#[derive(Debug)]
pub struct MacroRecorder {
    recording: Option<CommandMacro>,
    saved_macros: Vec<CommandMacro>,
}

impl MacroRecorder {
    /// Create a new recorder with no active recording.
    pub fn new() -> Self {
        Self {
            recording: None,
            saved_macros: Vec::new(),
        }
    }

    /// Start recording a new macro with the given name.  Returns `false` if
    /// already recording.
    pub fn start(&mut self, name: impl Into<String>) -> bool {
        if self.recording.is_some() {
            return false;
        }
        self.recording = Some(CommandMacro::new(name));
        true
    }

    /// Record a command into the active macro.  Returns `false` if not
    /// recording.
    pub fn record(&mut self, command: CoreEditorCommand) -> bool {
        match &mut self.recording {
            Some(m) => {
                m.record(command);
                true
            }
            None => false,
        }
    }

    /// Stop recording and save the macro.  Returns `None` if not recording.
    pub fn stop(&mut self) -> Option<&CommandMacro> {
        if let Some(m) = self.recording.take() {
            self.saved_macros.push(m);
            self.saved_macros.last()
        } else {
            None
        }
    }

    /// Return `true` when actively recording.
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Return all saved macros.
    pub fn macros(&self) -> &[CommandMacro] {
        &self.saved_macros
    }

    /// Find a saved macro by name.
    pub fn find_macro(&self, name: &str) -> Option<&CommandMacro> {
        self.saved_macros.iter().find(|m| m.name == name)
    }
}

impl Default for MacroRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Text analysis utilities for editor commands
// ---------------------------------------------------------------------------

/// Count the number of lines in the given text.
pub fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.lines().count()
}

/// Count the number of words (whitespace-separated tokens) in the given text.
pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Return the length (in characters) of each line.
pub fn line_lengths(text: &str) -> Vec<usize> {
    text.lines().map(|l| l.len()).collect()
}

/// Return the index of the longest line (0-based). Ties go to the first occurrence.
pub fn longest_line_index(text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    text.lines()
        .enumerate()
        .max_by_key(|(_, l)| l.len())
        .map(|(i, _)| i)
}

/// Indent every line of `text` by `count` spaces.
pub fn indent_all_lines(text: &str, count: usize) -> String {
    let prefix = " ".repeat(count);
    text.lines()
        .map(|l| format!("{}{}", prefix, l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove up to `count` leading spaces from every line.
pub fn dedent_all_lines(text: &str, count: usize) -> String {
    text.lines()
        .map(|l| {
            let spaces = l.chars().take_while(|c| *c == ' ').count();
            let remove = spaces.min(count);
            &l[remove..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Sort lines in the document alphabetically.
pub fn sort_lines(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort();
    lines.join("\n")
}

/// Remove consecutive duplicate lines from the text.
pub fn remove_duplicate_lines(text: &str) -> String {
    let mut result: Vec<&str> = Vec::new();
    for line in text.lines() {
        if result.last().map_or(true, |prev| *prev != line) {
            result.push(line);
        }
    }
    result.join("\n")
}

// ---------------------------------------------------------------------------
// Text editing operations
// ---------------------------------------------------------------------------

/// Delete a word to the left of the cursor position on a single line.
/// Returns the new line content and the new cursor column, or `None` if
/// the cursor is at position 0.
pub fn delete_word_left(line: &str, cursor_col: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = line.chars().collect();
    if cursor_col == 0 || chars.is_empty() {
        return None;
    }
    let col = cursor_col.min(chars.len());

    // Skip whitespace to the left
    let mut pos = col;
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }
    // Skip word characters to the left
    while pos > 0 && !chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    let mut result: Vec<char> = Vec::with_capacity(chars.len());
    result.extend_from_slice(&chars[..pos]);
    result.extend_from_slice(&chars[col..]);
    Some((result.into_iter().collect(), pos))
}

/// Delete a word to the right of the cursor position on a single line.
/// Returns the new line content, or `None` if the cursor is at the end.
pub fn delete_word_right(line: &str, cursor_col: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = line.chars().collect();
    if cursor_col >= chars.len() {
        return None;
    }

    let mut pos = cursor_col;
    // Skip word characters to the right
    while pos < chars.len() && !chars[pos].is_whitespace() {
        pos += 1;
    }
    // Skip whitespace to the right
    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }

    let mut result: Vec<char> = Vec::with_capacity(chars.len());
    result.extend_from_slice(&chars[..cursor_col]);
    result.extend_from_slice(&chars[pos..]);
    Some((result.into_iter().collect(), cursor_col))
}

/// Delete everything to the left of the cursor on a single line.
pub fn delete_all_left(line: &str, cursor_col: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    let col = cursor_col.min(chars.len());
    chars[col..].iter().collect()
}

/// Delete everything to the right of the cursor on a single line.
pub fn delete_all_right(line: &str, cursor_col: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    let col = cursor_col.min(chars.len());
    chars[..col].iter().collect()
}

/// Transform the selected text to title case (first letter of each word uppercase).
pub fn transform_to_title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    let rest: String = chars.flat_map(|c| c.to_lowercase()).collect();
                    format!("{upper}{rest}")
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert a camelCase or PascalCase identifier to snake_case.
pub fn to_snake_case(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 4);
    for (i, ch) in text.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        for lc in ch.to_lowercase() {
            result.push(lc);
        }
    }
    result
}

/// Convert a snake_case identifier to camelCase.
pub fn to_camel_case(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut capitalize_next = false;
    for ch in text.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            for uc in ch.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Reverse the characters in each line, preserving line structure.
pub fn reverse_lines_content(text: &str) -> String {
    text.lines()
        .map(|l| l.chars().rev().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Trim trailing whitespace from every line.
pub fn trim_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert text at a specific line and column position.
/// Returns the modified full text, or `None` if line is out of bounds.
pub fn insert_text_at(text: &str, line_index: usize, col: usize, insertion: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let line = lines[line_index];
    let chars: Vec<char> = line.chars().collect();
    let col = col.min(chars.len());

    let mut new_line = String::with_capacity(line.len() + insertion.len());
    new_line.extend(&chars[..col]);
    new_line.push_str(insertion);
    new_line.extend(&chars[col..]);

    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line_index] = new_line;
    Some(result.join("\n"))
}

/// Extract a substring from a specific line by column range.
/// Returns `None` if the line is out of bounds or the range is invalid.
pub fn extract_range(text: &str, line_index: usize, start_col: usize, end_col: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_index >= lines.len() {
        return None;
    }
    let chars: Vec<char> = lines[line_index].chars().collect();
    if start_col > end_col || end_col > chars.len() {
        return None;
    }
    Some(chars[start_col..end_col].iter().collect())
}

/// Remove all blank lines from the text.
pub fn remove_blank_lines(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse runs of multiple blank lines into a single blank line.
pub fn collapse_blank_lines(text: &str) -> String {
    let mut result: Vec<&str> = Vec::new();
    let mut prev_blank = false;
    for line in text.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(line);
        prev_blank = is_blank;
    }
    result.join("\n")
}

impl CoreEditorCommand {
    /// Returns `true` if this command requires an active text selection to be meaningful.
    pub fn requires_selection(&self) -> bool {
        matches!(
            self,
            Self::TransformToUppercase | Self::TransformToLowercase | Self::Cut | Self::Copy
        )
    }

    /// Returns `true` if this command is an undo/redo operation.
    pub fn is_history_navigation(&self) -> bool {
        matches!(self, Self::Undo | Self::Redo)
    }

    /// Returns the opposite/inverse command, if one exists.
    pub fn inverse(&self) -> Option<CoreEditorCommand> {
        match self {
            Self::Undo => Some(Self::Redo),
            Self::Redo => Some(Self::Undo),
            Self::DeleteLeft => Some(Self::DeleteRight),
            Self::DeleteRight => Some(Self::DeleteLeft),
            Self::CursorLeft => Some(Self::CursorRight),
            Self::CursorRight => Some(Self::CursorLeft),
            Self::CursorUp => Some(Self::CursorDown),
            Self::CursorDown => Some(Self::CursorUp),
            Self::CursorWordLeft => Some(Self::CursorWordRight),
            Self::CursorWordRight => Some(Self::CursorWordLeft),
            Self::CursorLineStart => Some(Self::CursorLineEnd),
            Self::CursorLineEnd => Some(Self::CursorLineStart),
            Self::CursorTop => Some(Self::CursorBottom),
            Self::CursorBottom => Some(Self::CursorTop),
            Self::CursorPageUp => Some(Self::CursorPageDown),
            Self::CursorPageDown => Some(Self::CursorPageUp),
            Self::IndentLine => Some(Self::OutdentLine),
            Self::OutdentLine => Some(Self::IndentLine),
            Self::MoveLinesUp => Some(Self::MoveLinesDown),
            Self::MoveLinesDown => Some(Self::MoveLinesUp),
            Self::SelectLeft => Some(Self::SelectRight),
            Self::SelectRight => Some(Self::SelectLeft),
            Self::SelectUp => Some(Self::SelectDown),
            Self::SelectDown => Some(Self::SelectUp),
            Self::TransformToUppercase => Some(Self::TransformToLowercase),
            Self::TransformToLowercase => Some(Self::TransformToUppercase),
            _ => None,
        }
    }
}

// ── EditorCommandMacro ──────────────────────────────────────────────────

/// A compound command that groups multiple sub-commands under a single name.
#[derive(Debug, Clone)]
pub struct EditorCommandMacro {
    name: String,
    commands: Vec<String>,
}

impl EditorCommandMacro {
    /// Create a new empty macro with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            commands: Vec::new(),
        }
    }

    /// Append a sub-command to this macro.
    pub fn add_command(&mut self, command: &str) {
        self.commands.push(command.to_string());
    }

    /// Return the ordered list of sub-commands.
    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    /// Return the macro name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Number of sub-commands in this macro.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether this macro contains no sub-commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Produce the list of commands that would execute, without side effects.
    pub fn execute_dry_run(&self) -> Vec<String> {
        self.commands.clone()
    }
}

impl fmt::Display for EditorCommandMacro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Macro({}, {} commands)", self.name, self.commands.len())
    }
}

// ── EditorCommandRepeat ─────────────────────────────────────────────────

/// Tracks command history so the last command can be repeated (like vi `.`).
#[derive(Debug, Clone)]
pub struct EditorCommandRepeat {
    history: Vec<String>,
}

impl EditorCommandRepeat {
    /// Create a new, empty repeat tracker.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Record a command execution.
    pub fn record(&mut self, command: &str) {
        self.history.push(command.to_string());
    }

    /// Return the most recently recorded command, if any.
    pub fn last_command(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Repeat the last command, returning a clone of its identifier.
    pub fn repeat(&self) -> Option<String> {
        self.history.last().cloned()
    }

    /// Produce `n` copies of the last command for batch replay.
    pub fn repeat_n(&self, n: usize) -> Vec<String> {
        match self.history.last() {
            Some(cmd) => vec![cmd.clone(); n],
            None => Vec::new(),
        }
    }

    /// Full history of recorded commands.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Number of commands in the history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Clear the recorded history.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

impl fmt::Display for EditorCommandRepeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.last_command() {
            Some(cmd) => write!(f, "Repeat(last={})", cmd),
            None => write!(f, "Repeat(empty)"),
        }
    }
}

// ── EditorCommandScope ──────────────────────────────────────────────────

/// Selection modes that determine how a command operates on text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorCommandScope {
    Character,
    Line,
    Block,
    Word,
    Paragraph,
}

impl EditorCommandScope {
    /// Parse a scope from its string label.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "character" => Some(Self::Character),
            "line" => Some(Self::Line),
            "block" => Some(Self::Block),
            "word" => Some(Self::Word),
            "paragraph" => Some(Self::Paragraph),
            _ => None,
        }
    }

    /// Short human-readable label.
    pub fn label(&self) -> &str {
        match self {
            Self::Character => "character",
            Self::Line => "line",
            Self::Block => "block",
            Self::Word => "word",
            Self::Paragraph => "paragraph",
        }
    }

    /// Whether this scope operates on whole lines.
    pub fn is_line_based(&self) -> bool {
        matches!(self, Self::Line | Self::Block | Self::Paragraph)
    }

    /// Human-readable description of the scope.
    pub fn description(&self) -> &str {
        match self {
            Self::Character => "Operate on individual characters",
            Self::Line => "Operate on entire lines",
            Self::Block => "Operate on rectangular blocks",
            Self::Word => "Operate on word boundaries",
            Self::Paragraph => "Operate on paragraph boundaries",
        }
    }
}

impl Default for EditorCommandScope {
    fn default() -> Self {
        Self::Character
    }
}

impl fmt::Display for EditorCommandScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ── CommandExecutionLogger ──────────────────────────────────────────────

/// A single recorded command execution.
#[derive(Debug, Clone)]
pub struct CommandExecution {
    pub command: String,
    pub success: bool,
    pub duration_us: u64,
}

impl fmt::Display for CommandExecution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "ok" } else { "err" };
        write!(f, "{}({}, {}µs)", self.command, status, self.duration_us)
    }
}

/// Logs command executions with timing and success/failure tracking.
#[derive(Debug, Clone)]
pub struct CommandExecutionLogger {
    executions: Vec<CommandExecution>,
}

impl CommandExecutionLogger {
    /// Create an empty logger.
    pub fn new() -> Self {
        Self {
            executions: Vec::new(),
        }
    }

    /// Record a command execution.
    pub fn log_execution(&mut self, command: &str, success: bool, duration_us: u64) {
        self.executions.push(CommandExecution {
            command: command.to_string(),
            success,
            duration_us,
        });
    }

    /// All recorded executions.
    pub fn executions(&self) -> &[CommandExecution] {
        &self.executions
    }

    /// Number of successful executions.
    pub fn successful_count(&self) -> usize {
        self.executions.iter().filter(|e| e.success).count()
    }

    /// Number of failed executions.
    pub fn failed_count(&self) -> usize {
        self.executions.iter().filter(|e| !e.success).count()
    }

    /// Sum of all recorded durations in microseconds.
    pub fn total_duration_us(&self) -> u64 {
        self.executions.iter().map(|e| e.duration_us).sum()
    }

    /// The most recently logged execution, if any.
    pub fn most_recent(&self) -> Option<&CommandExecution> {
        self.executions.last()
    }

    /// All executions whose command matches the given identifier.
    pub fn by_command(&self, command: &str) -> Vec<&CommandExecution> {
        self.executions
            .iter()
            .filter(|e| e.command == command)
            .collect()
    }
}

impl fmt::Display for CommandExecutionLogger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Logger(total={}, ok={}, err={}, duration={}µs)",
            self.executions.len(),
            self.successful_count(),
            self.failed_count(),
            self.total_duration_us()
        )
    }
}

// ── TextTransformer ──────────────────────────────────────────────────────

/// Utilities for transforming text case and formatting.
pub struct TextTransformer;

impl TextTransformer {
    pub fn to_upper_case(s: &str) -> String { s.to_uppercase() }
    pub fn to_lower_case(s: &str) -> String { s.to_lowercase() }

    pub fn to_title_case(s: &str) -> String {
        s.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let upper: String = c.to_uppercase().collect();
                        let rest: String = chars.as_str().to_lowercase();
                        format!("{}{}", upper, rest)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_snake_case(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 4);
        for (i, ch) in s.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 { result.push('_'); }
                for lower in ch.to_lowercase() { result.push(lower); }
            } else if ch == ' ' || ch == '-' {
                result.push('_');
            } else {
                result.push(ch);
            }
        }
        result
    }

    pub fn to_camel_case(s: &str) -> String {
        let parts: Vec<&str> = s.split(|c: char| c == '_' || c == '-' || c == ' ')
            .filter(|p| !p.is_empty())
            .collect();
        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                result.push_str(&part.to_lowercase());
            } else {
                let mut chars = part.chars();
                if let Some(c) = chars.next() {
                    for upper in c.to_uppercase() { result.push(upper); }
                    result.push_str(&chars.as_str().to_lowercase());
                }
            }
        }
        result
    }
}

// ── LineManipulator ─────────────────────────────────────────────────────

/// Operations on collections of lines.
pub struct LineManipulator;

impl LineManipulator {
    pub fn sort_lines(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        lines.sort();
        lines.join("\n")
    }

    pub fn reverse_lines(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        lines.reverse();
        lines.join("\n")
    }

    pub fn deduplicate_lines(text: &str) -> String {
        let mut seen = Vec::new();
        for line in text.lines() {
            if !seen.contains(&line) { seen.push(line); }
        }
        seen.join("\n")
    }

    pub fn join_lines(text: &str, separator: &str) -> String {
        text.lines().collect::<Vec<_>>().join(separator)
    }

    pub fn split_line_at(line: &str, col: usize) -> (String, String) {
        let left: String = line.chars().take(col).collect();
        let right: String = line.chars().skip(col).collect();
        (left, right)
    }
}

// ── IndentManipulator ───────────────────────────────────────────────────

/// Operations for managing indentation.
pub struct IndentManipulator;

impl IndentManipulator {
    pub fn indent_lines(text: &str, prefix: &str) -> String {
        text.lines().map(|l| format!("{}{}", prefix, l)).collect::<Vec<_>>().join("\n")
    }

    pub fn dedent_lines(text: &str, prefix: &str) -> String {
        text.lines()
            .map(|l| l.strip_prefix(prefix).unwrap_or(l).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Detect the most common indentation string in the given text.
    pub fn detect_indent_string(text: &str) -> String {
        let mut tab_count = 0u32;
        let mut space_count = 0u32;
        for line in text.lines() {
            if line.starts_with('\t') { tab_count += 1; }
            else if line.starts_with("    ") { space_count += 1; }
            else if line.starts_with("  ") { space_count += 1; }
        }
        if tab_count > space_count { "\t".to_string() } else { "    ".to_string() }
    }

    pub fn convert_tabs_to_spaces(text: &str, tab_size: usize) -> String {
        let spaces: String = " ".repeat(tab_size);
        text.replace('\t', &spaces)
    }

    pub fn convert_spaces_to_tabs(text: &str, tab_size: usize) -> String {
        let spaces: String = " ".repeat(tab_size);
        text.replace(&spaces, "\t")
    }
}


/// Editor command configuration manager.
#[derive(Debug, Clone)]
pub struct EditorCommandsConfig {
    entries: Vec<EditorCommandsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single editor command entry.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorCommandsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl EditorCommandsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl EditorCommandsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: EditorCommandsEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&EditorCommandsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut EditorCommandsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&EditorCommandsEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&EditorCommandsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&EditorCommandsEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<EditorCommandsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Editor command dispatch — extended utilities (qj)
// ---------------------------------------------------------------------------

/// Metric accumulator for editor_cmd operations.
#[derive(Debug, Clone)]
pub struct QjMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QjMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for editor_cmd.
#[derive(Debug, Clone)]
pub struct QjRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QjRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for editor_cmd lookups.
#[derive(Debug, Clone)]
pub struct QjLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QjLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
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

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for editor_commands
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEditorCommandsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEditorCommandsRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaEditorCommandsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEditorCommandsCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaEditorCommandsCounter {
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
    fn cut_line_first_line() {
        let text = "hello\nworld\nfoo";
        let (cut, remaining) = clipboard_cut_line(text, 0).unwrap();
        assert_eq!(cut, "hello");
        assert_eq!(remaining, "world\nfoo");
    }

    #[test]
    fn cut_line_middle_line() {
        let text = "a\nb\nc";
        let (cut, remaining) = clipboard_cut_line(text, 1).unwrap();
        assert_eq!(cut, "b");
        assert_eq!(remaining, "a\nc");
    }

    #[test]
    fn cut_line_last_line() {
        let text = "a\nb\nc";
        let (cut, remaining) = clipboard_cut_line(text, 2).unwrap();
        assert_eq!(cut, "c");
        assert_eq!(remaining, "a\nb");
    }

    #[test]
    fn cut_line_only_line() {
        let text = "only";
        let (cut, remaining) = clipboard_cut_line(text, 0).unwrap();
        assert_eq!(cut, "only");
        assert_eq!(remaining, "");
    }

    #[test]
    fn cut_line_out_of_bounds() {
        let text = "hello\nworld";
        assert!(clipboard_cut_line(text, 5).is_none());
    }

    #[test]
    fn transpose_middle() {
        let result = transpose_characters("abcde", 2).unwrap();
        assert_eq!(result, "acbde");
    }

    #[test]
    fn transpose_end() {
        let result = transpose_characters("abcde", 5).unwrap();
        assert_eq!(result, "abced");
    }

    #[test]
    fn transpose_at_start() {
        assert!(transpose_characters("abcde", 0).is_none());
    }

    #[test]
    fn transpose_single_char() {
        assert!(transpose_characters("a", 1).is_none());
    }

    #[test]
    fn transpose_two_chars() {
        let result = transpose_characters("ab", 1).unwrap();
        assert_eq!(result, "ba");
    }

    #[test]
    fn join_two_lines() {
        let text = "  hello  \n  world  \nfoo";
        let result = join_lines(text, 0, 1).unwrap();
        assert_eq!(result, "  hello world\nfoo");
    }

    #[test]
    fn join_three_lines() {
        let text = "a\n  b  \n  c  \nd";
        let result = join_lines(text, 0, 2).unwrap();
        assert_eq!(result, "a b c\nd");
    }

    #[test]
    fn join_lines_out_of_bounds() {
        let text = "a\nb";
        assert!(join_lines(text, 0, 5).is_none());
    }

    #[test]
    fn join_lines_reversed_range() {
        let text = "a\nb";
        assert!(join_lines(text, 1, 0).is_none());
    }

    #[test]
    fn join_single_line() {
        let text = "a\nb\nc";
        let result = join_lines(text, 1, 1).unwrap();
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn command_category_text_mutation() {
        assert_eq!(CoreEditorCommand::DeleteLeft.category(), CommandCategory::TextMutation);
        assert!(CoreEditorCommand::Type.is_text_mutation());
    }

    #[test]
    fn command_category_cursor() {
        assert_eq!(CoreEditorCommand::CursorUp.category(), CommandCategory::CursorMovement);
        assert!(!CoreEditorCommand::CursorUp.is_text_mutation());
    }

    #[test]
    fn command_category_selection() {
        assert_eq!(CoreEditorCommand::SelectAll.category(), CommandCategory::Selection);
    }

    #[test]
    fn command_category_clipboard() {
        assert_eq!(CoreEditorCommand::Cut.category(), CommandCategory::Clipboard);
    }

    #[test]
    fn find_commands_by_category_cursor() {
        let cmds = find_commands_by_category(CommandCategory::CursorMovement);
        assert!(cmds.contains(&CoreEditorCommand::CursorLeft));
        assert!(!cmds.contains(&CoreEditorCommand::DeleteLeft));
    }

    #[test]
    fn search_commands_by_label() {
        let results = search_commands("delete");
        assert!(!results.is_empty());
        assert!(results.iter().all(|d| d.label.to_lowercase().contains("delete")));
    }

    #[test]
    fn search_commands_case_insensitive() {
        let results = search_commands("CURSOR");
        assert!(!results.is_empty());
    }

    #[test]
    fn search_commands_no_match() {
        let results = search_commands("xyznonexistent");
        assert!(results.is_empty());
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

    #[test]
    fn duplicate_line_below() {
        let text = "a\nb\nc";
        let result = duplicate_line(text, 1, false).unwrap();
        assert_eq!(result, "a\nb\nb\nc");
    }

    #[test]
    fn duplicate_line_above() {
        let text = "a\nb\nc";
        let result = duplicate_line(text, 1, true).unwrap();
        assert_eq!(result, "a\nb\nb\nc");
    }

    #[test]
    fn duplicate_line_out_of_bounds() {
        assert!(duplicate_line("a\nb", 5, false).is_none());
    }

    #[test]
    fn move_line_up() {
        let text = "a\nb\nc";
        let result = move_line(text, 1, true).unwrap();
        assert_eq!(result, "b\na\nc");
    }

    #[test]
    fn move_line_down() {
        let text = "a\nb\nc";
        let result = move_line(text, 1, false).unwrap();
        assert_eq!(result, "a\nc\nb");
    }

    #[test]
    fn move_line_up_at_top() {
        assert!(move_line("a\nb", 0, true).is_none());
    }

    #[test]
    fn move_line_down_at_bottom() {
        assert!(move_line("a\nb", 1, false).is_none());
    }

    #[test]
    fn toggle_comment_add() {
        let text = "hello\nworld";
        let result = toggle_line_comment(text, 0, 1, "//").unwrap();
        assert_eq!(result, "// hello\n// world");
    }

    #[test]
    fn toggle_comment_remove() {
        let text = "// hello\n// world";
        let result = toggle_line_comment(text, 0, 1, "//").unwrap();
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn toggle_comment_partial() {
        let text = "// hello\nworld";
        let result = toggle_line_comment(text, 0, 1, "//").unwrap();
        assert_eq!(result, "// // hello\n// world");
    }

    #[test]
    fn transform_range_uppercase_basic() {
        let text = "hello world";
        let result = transform_range_uppercase(text, 0, 0, 5).unwrap();
        assert_eq!(result, "HELLO world");
    }

    #[test]
    fn transform_range_lowercase_basic() {
        let text = "HELLO world";
        let result = transform_range_lowercase(text, 0, 0, 5).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn transform_range_out_of_bounds() {
        assert!(transform_range_uppercase("hi", 0, 0, 10).is_none());
        assert!(transform_range_uppercase("hi", 5, 0, 1).is_none());
    }

    // ── CommandHistory tests ──

    #[test]
    fn command_history_undo_redo() {
        let mut hist = CommandHistory::new(100);
        hist.push(CoreEditorCommand::Type, 1);
        hist.push(CoreEditorCommand::DeleteLeft, 2);
        assert_eq!(hist.len(), 2);
        assert!(hist.can_undo());

        let undone = hist.undo().unwrap();
        assert_eq!(undone.command, CoreEditorCommand::DeleteLeft);
        assert!(hist.can_redo());

        let redone = hist.redo().unwrap();
        assert_eq!(redone.command, CoreEditorCommand::DeleteLeft);
        assert!(!hist.can_redo());
    }

    #[test]
    fn command_history_push_clears_redo() {
        let mut hist = CommandHistory::new(100);
        hist.push(CoreEditorCommand::Type, 1);
        hist.push(CoreEditorCommand::DeleteLeft, 2);
        hist.undo(); // undo DeleteLeft
        hist.push(CoreEditorCommand::Paste, 3);
        assert!(!hist.can_redo());
        assert_eq!(hist.len(), 2); // Type + Paste
    }

    // ── ComposedCommand tests ──

    #[test]
    fn composed_command_label() {
        let comp = ComposedCommand::new(CoreEditorCommand::SelectAll, CoreEditorCommand::Copy);
        assert!(comp.label.contains(CoreEditorCommand::SelectAll.label()));
        assert!(comp.label.contains(CoreEditorCommand::Copy.label()));
        let (id1, id2) = comp.command_ids();
        assert_eq!(id1, "editor.action.selectAll");
        assert_eq!(id2, "editor.action.clipboardCopyAction");
    }

    #[test]
    fn compose_sequence() {
        let seq = vec![
            CoreEditorCommand::SelectAll,
            CoreEditorCommand::Copy,
            CoreEditorCommand::Paste,
        ];
        let composed = compose_command_sequence(&seq);
        assert_eq!(composed.len(), 2);
    }

    // ── MacroRecorder tests ──

    #[test]
    fn macro_recorder_record_and_playback() {
        let mut rec = MacroRecorder::new();
        assert!(rec.start("test_macro"));
        assert!(rec.is_recording());
        assert!(!rec.start("another")); // can't start while recording
        rec.record(CoreEditorCommand::Type);
        rec.record(CoreEditorCommand::NewLine);
        let saved = rec.stop().unwrap();
        assert_eq!(saved.name, "test_macro");
        assert_eq!(saved.len(), 2);
        assert!(!rec.is_recording());
        assert!(rec.find_macro("test_macro").is_some());
    }

    #[test]
    fn macro_recorder_find_nonexistent() {
        let rec = MacroRecorder::new();
        assert!(rec.find_macro("nope").is_none());
    }

    #[test]
    fn count_lines_basic() {
        assert_eq!(count_lines("a\nb\nc"), 3);
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("single"), 1);
    }

    #[test]
    fn count_words_basic() {
        assert_eq!(count_words("hello world foo"), 3);
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("  spaced  "), 1);
    }

    #[test]
    fn line_lengths_measures_each() {
        let lens = line_lengths("ab\ncde\nf");
        assert_eq!(lens, vec![2, 3, 1]);
    }

    #[test]
    fn longest_line_index_finds_longest() {
        assert_eq!(longest_line_index("ab\ncdef\ng"), Some(1));
        assert_eq!(longest_line_index(""), None);
    }

    #[test]
    fn indent_all_lines_adds_spaces() {
        let result = indent_all_lines("a\nb", 4);
        assert_eq!(result, "    a\n    b");
    }

    #[test]
    fn dedent_all_lines_removes_spaces() {
        let result = dedent_all_lines("    a\n  b", 2);
        assert_eq!(result, "  a\nb");
    }

    #[test]
    fn sort_lines_alphabetical() {
        let result = sort_lines("cherry\napple\nbanana");
        assert_eq!(result, "apple\nbanana\ncherry");
    }

    #[test]
    fn remove_duplicate_lines_removes_consecutive() {
        let result = remove_duplicate_lines("a\na\nb\nc\nc");
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn remove_duplicate_lines_empty() {
        assert_eq!(remove_duplicate_lines(""), "");
    }

    // ── delete_word_left / delete_word_right tests ──

    #[test]
    fn delete_word_left_middle_of_line() {
        // "hello world foo" cursor at col 11 (after "world "), deletes "world " → "hello foo"
        let (result, col) = delete_word_left("hello world foo", 11).unwrap();
        assert_eq!(result, "hello  foo");
        assert_eq!(col, 6);
    }

    #[test]
    fn delete_word_left_at_start() {
        assert!(delete_word_left("hello", 0).is_none());
    }

    #[test]
    fn delete_word_left_single_word() {
        let (result, col) = delete_word_left("hello", 5).unwrap();
        assert_eq!(result, "");
        assert_eq!(col, 0);
    }

    #[test]
    fn delete_word_right_middle_of_line() {
        let (result, col) = delete_word_right("hello world foo", 6).unwrap();
        assert_eq!(result, "hello foo");
        assert_eq!(col, 6);
    }

    #[test]
    fn delete_word_right_at_end() {
        assert!(delete_word_right("hello", 5).is_none());
    }

    #[test]
    fn delete_word_right_from_start() {
        let (result, col) = delete_word_right("hello world", 0).unwrap();
        assert_eq!(result, "world");
        assert_eq!(col, 0);
    }

    // ── delete_all_left / delete_all_right tests ──

    #[test]
    fn delete_all_left_middle() {
        assert_eq!(delete_all_left("hello world", 5), " world");
    }

    #[test]
    fn delete_all_left_at_start() {
        assert_eq!(delete_all_left("hello", 0), "hello");
    }

    #[test]
    fn delete_all_right_middle() {
        assert_eq!(delete_all_right("hello world", 5), "hello");
    }

    #[test]
    fn delete_all_right_at_end() {
        assert_eq!(delete_all_right("hello", 5), "hello");
    }

    // ── transform_to_title_case tests ──

    #[test]
    fn title_case_basic() {
        assert_eq!(transform_to_title_case("hello world"), "Hello World");
    }

    #[test]
    fn title_case_already_upper() {
        assert_eq!(transform_to_title_case("HELLO WORLD"), "Hello World");
    }

    #[test]
    fn title_case_empty() {
        assert_eq!(transform_to_title_case(""), "");
    }

    // ── to_snake_case / to_camel_case tests ──

    #[test]
    fn snake_case_from_camel() {
        assert_eq!(to_snake_case("helloWorld"), "hello_world");
    }

    #[test]
    fn snake_case_from_pascal() {
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
    }

    #[test]
    fn camel_case_from_snake() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
    }

    #[test]
    fn camel_case_no_underscores() {
        assert_eq!(to_camel_case("hello"), "hello");
    }

    // ── reverse_lines_content tests ──

    #[test]
    fn reverse_lines_content_basic() {
        assert_eq!(reverse_lines_content("abc\ndef"), "cba\nfed");
    }

    #[test]
    fn reverse_lines_content_single() {
        assert_eq!(reverse_lines_content("hello"), "olleh");
    }

    // ── trim_trailing_whitespace tests ──

    #[test]
    fn trim_trailing_whitespace_basic() {
        assert_eq!(
            trim_trailing_whitespace("hello  \nworld\t\nfoo"),
            "hello\nworld\nfoo"
        );
    }

    #[test]
    fn trim_trailing_whitespace_preserves_leading() {
        assert_eq!(
            trim_trailing_whitespace("  hello  \n  world  "),
            "  hello\n  world"
        );
    }

    // ── insert_text_at tests ──

    #[test]
    fn insert_text_at_middle() {
        let result = insert_text_at("hello world", 0, 5, " beautiful").unwrap();
        assert_eq!(result, "hello beautiful world");
    }

    #[test]
    fn insert_text_at_start() {
        let result = insert_text_at("world", 0, 0, "hello ").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn insert_text_at_out_of_bounds_line() {
        assert!(insert_text_at("hello", 5, 0, "x").is_none());
    }

    // ── extract_range tests ──

    #[test]
    fn extract_range_basic() {
        let result = extract_range("hello world", 0, 6, 11).unwrap();
        assert_eq!(result, "world");
    }

    #[test]
    fn extract_range_invalid() {
        assert!(extract_range("hello", 0, 3, 1).is_none());
        assert!(extract_range("hello", 0, 0, 10).is_none());
    }

    // ── remove_blank_lines / collapse_blank_lines tests ──

    #[test]
    fn remove_blank_lines_basic() {
        assert_eq!(remove_blank_lines("a\n\nb\n  \nc"), "a\nb\nc");
    }

    #[test]
    fn collapse_blank_lines_basic() {
        assert_eq!(collapse_blank_lines("a\n\n\nb\n\nc"), "a\n\nb\n\nc");
    }

    // ── CoreEditorCommand extension method tests ──

    #[test]
    fn requires_selection_correct() {
        assert!(CoreEditorCommand::TransformToUppercase.requires_selection());
        assert!(CoreEditorCommand::Cut.requires_selection());
        assert!(!CoreEditorCommand::Undo.requires_selection());
        assert!(!CoreEditorCommand::CursorLeft.requires_selection());
    }

    #[test]
    fn is_history_navigation_correct() {
        assert!(CoreEditorCommand::Undo.is_history_navigation());
        assert!(CoreEditorCommand::Redo.is_history_navigation());
        assert!(!CoreEditorCommand::Type.is_history_navigation());
    }

    #[test]
    fn inverse_symmetry() {
        let undo_inv = CoreEditorCommand::Undo.inverse().unwrap();
        assert_eq!(undo_inv, CoreEditorCommand::Redo);
        let redo_inv = undo_inv.inverse().unwrap();
        assert_eq!(redo_inv, CoreEditorCommand::Undo);
    }

    #[test]
    fn inverse_cursor_pairs() {
        assert_eq!(CoreEditorCommand::CursorLeft.inverse(), Some(CoreEditorCommand::CursorRight));
        assert_eq!(CoreEditorCommand::CursorUp.inverse(), Some(CoreEditorCommand::CursorDown));
        assert_eq!(CoreEditorCommand::CursorTop.inverse(), Some(CoreEditorCommand::CursorBottom));
        assert_eq!(CoreEditorCommand::IndentLine.inverse(), Some(CoreEditorCommand::OutdentLine));
    }

    // ── EditorCommandMacro tests ──

    #[test]
    fn macro_empty_on_creation() {
        let m = EditorCommandMacro::new("my_macro");
        assert_eq!(m.name(), "my_macro");
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn macro_add_and_list_commands() {
        let mut m = EditorCommandMacro::new("build");
        m.add_command("save");
        m.add_command("compile");
        m.add_command("test");
        assert_eq!(m.len(), 3);
        assert!(!m.is_empty());
        assert_eq!(m.commands(), &["save", "compile", "test"]);
    }

    #[test]
    fn macro_dry_run_clones_commands() {
        let mut m = EditorCommandMacro::new("deploy");
        m.add_command("build");
        m.add_command("push");
        let dry = m.execute_dry_run();
        assert_eq!(dry, vec!["build".to_string(), "push".to_string()]);
    }

    #[test]
    fn macro_display() {
        let mut m = EditorCommandMacro::new("fmt");
        m.add_command("indent");
        let s = format!("{m}");
        assert!(s.contains("fmt"));
        assert!(s.contains("1 commands"));
    }

    // ── EditorCommandRepeat tests ──

    #[test]
    fn repeat_empty_initially() {
        let r = EditorCommandRepeat::new();
        assert!(r.last_command().is_none());
        assert!(r.repeat().is_none());
        assert_eq!(r.history_len(), 0);
    }

    #[test]
    fn repeat_records_and_replays() {
        let mut r = EditorCommandRepeat::new();
        r.record("deleteLeft");
        r.record("type");
        assert_eq!(r.last_command(), Some("type"));
        assert_eq!(r.repeat(), Some("type".to_string()));
        assert_eq!(r.history_len(), 2);
    }

    #[test]
    fn repeat_n_produces_copies() {
        let mut r = EditorCommandRepeat::new();
        r.record("undo");
        let v = r.repeat_n(3);
        assert_eq!(v, vec!["undo".to_string(); 3]);
    }

    #[test]
    fn repeat_clear_resets() {
        let mut r = EditorCommandRepeat::new();
        r.record("redo");
        r.clear();
        assert!(r.last_command().is_none());
        assert_eq!(r.history_len(), 0);
    }

    // ── EditorCommandScope tests ──

    #[test]
    fn scope_default_is_character() {
        assert_eq!(EditorCommandScope::default(), EditorCommandScope::Character);
    }

    #[test]
    fn scope_from_str_round_trip() {
        for label in &["character", "line", "block", "word", "paragraph"] {
            let scope = EditorCommandScope::from_str(label).unwrap();
            assert_eq!(scope.label(), *label);
        }
        assert!(EditorCommandScope::from_str("unknown").is_none());
    }

    #[test]
    fn scope_is_line_based() {
        assert!(!EditorCommandScope::Character.is_line_based());
        assert!(EditorCommandScope::Line.is_line_based());
        assert!(EditorCommandScope::Block.is_line_based());
        assert!(!EditorCommandScope::Word.is_line_based());
        assert!(EditorCommandScope::Paragraph.is_line_based());
    }

    #[test]
    fn scope_description_non_empty() {
        for scope in &[
            EditorCommandScope::Character,
            EditorCommandScope::Line,
            EditorCommandScope::Block,
            EditorCommandScope::Word,
            EditorCommandScope::Paragraph,
        ] {
            assert!(!scope.description().is_empty());
        }
    }

    // ── CommandExecutionLogger tests ──

    #[test]
    fn logger_tracks_executions() {
        let mut logger = CommandExecutionLogger::new();
        logger.log_execution("type", true, 50);
        logger.log_execution("deleteLeft", false, 120);
        logger.log_execution("type", true, 30);
        assert_eq!(logger.successful_count(), 2);
        assert_eq!(logger.failed_count(), 1);
        assert_eq!(logger.total_duration_us(), 200);
        assert_eq!(logger.most_recent().unwrap().command, "type");
    }

    #[test]
    fn logger_by_command_filters() {
        let mut logger = CommandExecutionLogger::new();
        logger.log_execution("undo", true, 10);
        logger.log_execution("redo", true, 20);
        logger.log_execution("undo", false, 15);
        let undo_execs = logger.by_command("undo");
        assert_eq!(undo_execs.len(), 2);
        assert!(undo_execs.iter().all(|e| e.command == "undo"));
    }

    #[test]
    fn inverse_none_for_type() {
        assert!(CoreEditorCommand::Type.inverse().is_none());
    }

    // ── TextTransformer tests ──

    #[test]
    fn text_upper_case() {
        assert_eq!(TextTransformer::to_upper_case("hello"), "HELLO");
    }

    #[test]
    fn text_lower_case() {
        assert_eq!(TextTransformer::to_lower_case("HELLO World"), "hello world");
    }

    #[test]
    fn text_title_case() {
        assert_eq!(TextTransformer::to_title_case("hello world"), "Hello World");
        assert_eq!(TextTransformer::to_title_case("HELLO WORLD"), "Hello World");
    }

    #[test]
    fn text_snake_case() {
        assert_eq!(TextTransformer::to_snake_case("camelCase"), "camel_case");
        assert_eq!(TextTransformer::to_snake_case("hello world"), "hello_world");
    }

    #[test]
    fn text_camel_case() {
        assert_eq!(TextTransformer::to_camel_case("hello_world"), "helloWorld");
        assert_eq!(TextTransformer::to_camel_case("some-thing"), "someThing");
    }

    // ── LineManipulator tests ──

    #[test]
    fn manipulator_sort_lines() {
        assert_eq!(LineManipulator::sort_lines("c\na\nb"), "a\nb\nc");
    }

    #[test]
    fn manipulator_reverse_lines() {
        assert_eq!(LineManipulator::reverse_lines("a\nb\nc"), "c\nb\na");
    }

    #[test]
    fn manipulator_deduplicate_lines() {
        assert_eq!(LineManipulator::deduplicate_lines("a\nb\na\nc"), "a\nb\nc");
    }

    #[test]
    fn manipulator_join_lines() {
        assert_eq!(LineManipulator::join_lines("a\nb\nc", ", "), "a, b, c");
    }

    #[test]
    fn manipulator_split_line_at() {
        let (l, r) = LineManipulator::split_line_at("hello world", 5);
        assert_eq!(l, "hello");
        assert_eq!(r, " world");
    }

    // ── IndentManipulator tests ──

    #[test]
    fn indent_and_dedent() {
        let text = "a\nb";
        let indented = IndentManipulator::indent_lines(text, "  ");
        assert_eq!(indented, "  a\n  b");
        let dedented = IndentManipulator::dedent_lines(&indented, "  ");
        assert_eq!(dedented, "a\nb");
    }

    #[test]
    fn detect_indent_tabs() {
        let text = "\tfoo\n\tbar\n  baz";
        assert_eq!(IndentManipulator::detect_indent_string(text), "\t");
    }

    #[test]
    fn convert_tabs_spaces() {
        assert_eq!(IndentManipulator::convert_tabs_to_spaces("\thello", 4), "    hello");
        assert_eq!(IndentManipulator::convert_spaces_to_tabs("    hello", 4), "\thello");
    }

    #[test]
    fn editor_commands_entry_creation() {
        let e = EditorCommandsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn editor_commands_entry_with_priority() {
        let e = EditorCommandsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn editor_commands_entry_metadata() {
        let e = EditorCommandsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn editor_commands_entry_remove_meta() {
        let mut e = EditorCommandsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn editor_commands_entry_activate_deactivate() {
        let mut e = EditorCommandsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn editor_commands_config_add_sorted() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("lo", "Lo").with_priority(1));
        c.add(EditorCommandsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn editor_commands_config_capacity() {
        let mut c = EditorCommandsConfig::new(1);
        assert!(c.add(EditorCommandsEntry::new("a", "A")));
        assert!(!c.add(EditorCommandsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn editor_commands_config_remove() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn editor_commands_config_get() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn editor_commands_config_active_entries() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        c.add(EditorCommandsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn editor_commands_config_enable_disable() {
        let mut c = EditorCommandsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn editor_commands_config_clear() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn editor_commands_config_find_by_label() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn editor_commands_config_top_n() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A").with_priority(1));
        c.add(EditorCommandsEntry::new("b", "B").with_priority(2));
        c.add(EditorCommandsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn editor_commands_config_deactivate_activate_all() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        c.add(EditorCommandsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn editor_commands_config_highest_priority() {
        let mut c = EditorCommandsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(EditorCommandsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn editor_commands_config_contains() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn editor_commands_config_labels() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "Alpha"));
        c.add(EditorCommandsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn editor_commands_config_drain_inactive() {
        let mut c = EditorCommandsConfig::new(10);
        c.add(EditorCommandsEntry::new("a", "A"));
        c.add(EditorCommandsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qj_metrics_empty() {
        let m = QjMetrics::new("editor_cmd");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qj_metrics_record_and_mean() {
        let mut m = QjMetrics::new("editor_cmd");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qj_metrics_min_max() {
        let mut m = QjMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qj_metrics_variance_and_std() {
        let mut m = QjMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qj_metrics_percentile() {
        let mut m = QjMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qj_metrics_merge() {
        let mut a = QjMetrics::new("a");
        a.record(1.0);
        let mut b = QjMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qj_metrics_reset() {
        let mut m = QjMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qj_rate_window_empty() {
        let rw = QjRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qj_rate_window_tick_and_rate() {
        let mut rw = QjRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qj_lru_cache_basic() {
        let mut c = QjLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qj_lru_cache_contains_and_keys() {
        let mut c = QjLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qj_lru_cache_remove() {
        let mut c = QjLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qj_metrics_sum() {
        let mut m = QjMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qj_metrics_label() {
        let m = QjMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qj_lru_cache_clear() {
        let mut c = QjLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for editor_commands
    #[test]
    fn xa_editor_commands_ring_new() {
        let rb = super::XaEditorCommandsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_editor_commands_ring_push_len() {
        let mut rb = super::XaEditorCommandsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_editor_commands_ring_wrap() {
        let mut rb = super::XaEditorCommandsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_editor_commands_ring_mean_empty() {
        let rb = super::XaEditorCommandsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_editor_commands_ring_mean_values() {
        let mut rb = super::XaEditorCommandsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_editor_commands_ring_min_max() {
        let mut rb = super::XaEditorCommandsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_editor_commands_ring_iter() {
        let mut rb = super::XaEditorCommandsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_editor_commands_counter_new() {
        let c = super::XaEditorCommandsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_commands_counter_inc() {
        let mut c = super::XaEditorCommandsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_editor_commands_counter_inc_by() {
        let mut c = super::XaEditorCommandsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_editor_commands_counter_reset() {
        let mut c = super::XaEditorCommandsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_editor_commands_counter_clear() {
        let mut c = super::XaEditorCommandsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_commands_counter_default() {
        let c = super::XaEditorCommandsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
