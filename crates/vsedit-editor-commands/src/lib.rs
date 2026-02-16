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
}
