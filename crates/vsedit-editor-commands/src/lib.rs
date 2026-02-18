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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 33
// ---------------------------------------------------------------------------

/// Generic object pool `Xc33Pool<T>`.
pub struct Xc33Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc33Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc33PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc33Pool<T> {
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
    pub fn stats(&self) -> Xc33PoolStats {
        Xc33PoolStats {
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

impl<T> Default for Xc33Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc33Scheduler`.
pub struct Xc33Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc33Scheduler {
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

impl Default for Xc33Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_33 hash for the given byte slice.
pub fn xc_33_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_33 convention.
pub fn xc_33_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe10 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe10Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe10PipelineError {
    pub stage: Xe10Stage,
    pub message: String,
}

impl std::fmt::Display for Xe10PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe10Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe10Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError>>>,
    stage_names: Vec<Xe10Stage>,
}

impl Xe10Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe10Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe10Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe10Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe10Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
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

    pub fn compose(mut self, other: Xe10Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe10CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe10CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe10Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe10CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe10CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe10Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe10CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_10_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe10CacheEntry {
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

    fn xe_10_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe10CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_10_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
    Ok(data)
}

pub fn xe_10_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_10_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_10_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_10_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe10PipelineError> {
    Err(Xe10PipelineError {
        stage: Xe10Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #74
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf74Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf74TrieNode {
    children: std::collections::HashMap<char, Xf74TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf74Trie {
    root: Xf74TrieNode,
    count: usize,
}

impl Xf74Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf74TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf74TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf74TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf74BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf74BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 32).
pub struct Xh32SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh32SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 74 as u64,
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

/// A compact bit set supporting boolean operations (variant 32).
pub struct Xh32BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh32BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 32).
pub struct Xi32Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi32Deque<T> {
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
pub struct Xi32Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi32Interval {
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

/// A simple interval tree (variant 32).
pub struct Xi32IntervalTree {
    xi_intervals: Vec<Xi32Interval>,
}

impl Xi32IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi32Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi32Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi32Interval) -> Vec<&Xi32Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi32Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi32Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi32Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi32Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi32Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi32Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 32) ---

/// Disjoint set / union-find for crate 32.
pub struct Xj32UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj32UnionFind {
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

const XJ32_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 32.
pub struct Xj32BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj32BTreeNode<K, V>>>,
    len: usize,
}

struct Xj32BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj32BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj32BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ32_BTREE_ORDER - 1
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
        let mid = XJ32_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj32BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj32BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj32BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj32BTreeNode::xj_new_leaf();
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


// --- xk_32 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk32SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk32SegmentTree {
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
pub struct Xk32DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk32DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_32).
#[derive(Debug, Clone)]
pub struct Xl32Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl32Rope {
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

/// Suffix array for efficient string searching (xl_32).
#[derive(Debug, Clone)]
pub struct Xl32SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl32SuffixArray {
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
pub struct Xm32MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm32MatrixSparse {
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
pub struct Xm32Tokenizer {
    text: String,
}

impl Xm32Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 32.
pub struct Xn32Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn32Fenwick {
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

// ----- AVL tree map — crate 32 -----

#[derive(Debug, Clone)]
struct Xn32AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn32AvlNode<K, V>>>,
    right: Option<Box<Xn32AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 32.
#[derive(Debug, Clone)]
pub struct Xn32AVL<K, V> {
    root: Option<Box<Xn32AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn32AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn32AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn32AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn32AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn32AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn32AvlNode<K, V>>) -> Box<Xn32AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn32AvlNode<K, V>>) -> Box<Xn32AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn32AvlNode<K, V>>) -> Box<Xn32AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn32AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn32AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn32AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn32AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn32AvlNode<K, V>>) -> &Xn32AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn32AvlNode<K, V>>) -> (Box<Xn32AvlNode<K, V>>, Option<Box<Xn32AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn32AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn32AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn32AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn32AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn32AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn32AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn32AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


    // ---- xc_ pool / scheduler tests – block 33 ----

    #[test]
    fn xc_33_pool_new_empty() {
        let pool: super::Xc33Pool<i32> = super::Xc33Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_33_pool_release_acquire() {
        let mut pool = super::Xc33Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_33_pool_acquire_empty() {
        let mut pool: super::Xc33Pool<i32> = super::Xc33Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_33_pool_full() {
        let mut pool = super::Xc33Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_33_pool_drain() {
        let mut pool = super::Xc33Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_33_pool_stats() {
        let mut pool = super::Xc33Pool::new(8);
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
    fn xc_33_pool_clear() {
        let mut pool = super::Xc33Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_33_pool_shrink() {
        let mut pool = super::Xc33Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_33_pool_default() {
        let pool: super::Xc33Pool<String> = super::Xc33Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_33_pool_extend() {
        let mut pool = super::Xc33Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_33_pool_retain() {
        let mut pool = super::Xc33Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_33_scheduler_round_robin() {
        let mut sched = super::Xc33Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_33_scheduler_empty() {
        let mut sched = super::Xc33Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_33_scheduler_reset() {
        let mut sched = super::Xc33Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_33_scheduler_add_remove() {
        let mut sched = super::Xc33Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_33_scheduler_targets() {
        let sched = super::Xc33Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_33_hash_empty() {
        assert_eq!(super::xc_33_hash(b""), 5381);
    }

    #[test]
    fn xc_33_hash_data() {
        let h = super::xc_33_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_33_hash(b"hello"), h);
    }

    #[test]
    fn xc_33_reverse_str() {
        assert_eq!(super::xc_33_reverse("abc"), "cba");
        assert_eq!(super::xc_33_reverse(""), "");
    }


    #[test]
    fn xe_10_pipeline_empty() {
        let p = super::Xe10Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_10_pipeline_parse_stage() {
        let p = super::Xe10Pipeline::new()
            .add_parse(super::xe_10_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_10_pipeline_transform_double() {
        let p = super::Xe10Pipeline::new()
            .add_transform(super::xe_10_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_10_pipeline_validate_reverse() {
        let p = super::Xe10Pipeline::new()
            .add_validate(super::xe_10_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_10_pipeline_emit_filter() {
        let p = super::Xe10Pipeline::new()
            .add_emit(super::xe_10_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_10_pipeline_multi_stage() {
        let p = super::Xe10Pipeline::new()
            .add_parse(super::xe_10_pipeline_identity)
            .add_transform(super::xe_10_pipeline_double)
            .add_validate(super::xe_10_pipeline_reverse)
            .add_emit(super::xe_10_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_10_pipeline_error_propagation() {
        let p = super::Xe10Pipeline::new()
            .add_parse(super::xe_10_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe10Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_10_pipeline_compose() {
        let p1 = super::Xe10Pipeline::new()
            .add_parse(super::xe_10_pipeline_identity);
        let p2 = super::Xe10Pipeline::new()
            .add_transform(super::xe_10_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_10_pipeline_error_display() {
        let e = super::Xe10PipelineError {
            stage: super::Xe10Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_10_cache_put_get() {
        let mut c = super::Xe10Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_10_cache_miss() {
        let mut c: super::Xe10Cache<&str, i32> = super::Xe10Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_10_cache_ttl_expiry() {
        let mut c = super::Xe10Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_10_cache_evict() {
        let mut c = super::Xe10Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_10_cache_capacity() {
        let mut c = super::Xe10Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_10_cache_stats() {
        let mut c = super::Xe10Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_10_cache_clear() {
        let mut c = super::Xe10Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #74 --

    #[test]
    fn xf74_trie_insert_search() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf74_trie_starts_with() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf74_trie_remove() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf74_trie_word_count() {
        let mut t = Xf74Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf74_trie_longest_prefix() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf74_trie_all_words() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf74_trie_autocomplete() {
        let mut t = Xf74Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf74_trie_empty_search() {
        let t = Xf74Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf74_bloom_add_contains() {
        let mut bf = Xf74BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf74_bloom_probably_absent() {
        let bf = Xf74BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf74_bloom_false_positive_rate() {
        let mut bf = Xf74BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf74_bloom_clear() {
        let mut bf = Xf74BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf74_bloom_union() {
        let mut a = Xf74BloomFilter::xf_new(512, 2);
        let mut b = Xf74BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf74_bloom_intersection_estimate() {
        let mut a = Xf74BloomFilter::xf_new(512, 2);
        let mut b = Xf74BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf74_bloom_union_size_mismatch() {
        let a = Xf74BloomFilter::xf_new(256, 2);
        let b = Xf74BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh32_skip_insert_contains() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh32_skip_remove() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh32_skip_len() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh32_skip_range_query() {
        let mut sl = super::Xh32SkipList::xh_new(4);
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
    fn xh32_skip_floor_ceiling() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh32_skip_rank() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh32_skip_empty() {
        let sl = super::Xh32SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh32_skip_duplicates() {
        let mut sl = super::Xh32SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh32_bitset_set_test() {
        let mut bs = super::Xh32BitSet::xh_new(256);
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
    fn xh32_bitset_clear_count() {
        let mut bs = super::Xh32BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh32_bitset_and_or_xor() {
        let mut a = super::Xh32BitSet::xh_new(128);
        let mut b = super::Xh32BitSet::xh_new(128);
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
    fn xh32_bitset_iter_ones() {
        let mut bs = super::Xh32BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh32_bitset_first_last() {
        let mut bs = super::Xh32BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh32_bitset_empty() {
        let bs = super::Xh32BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi32_deque_push_pop_back() {
        let mut dq = super::Xi32Deque::xi_new(4);
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
    fn xi32_deque_push_pop_front() {
        let mut dq = super::Xi32Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi32_deque_mixed_ops() {
        let mut dq = super::Xi32Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi32_deque_get_and_split() {
        let mut dq = super::Xi32Deque::xi_new(8);
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
    fn xi32_deque_rotate_left() {
        let mut dq = super::Xi32Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi32_deque_rotate_right() {
        let mut dq = super::Xi32Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi32_deque_grow() {
        let mut dq = super::Xi32Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi32_deque_empty() {
        let dq = super::Xi32Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi32_interval_tree_insert_query() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi32Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi32Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi32_interval_tree_overlap() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi32Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi32Interval::xi_new(12, 20));
        let q = super::Xi32Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi32_interval_tree_remove() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi32Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi32_interval_tree_gaps() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi32Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi32Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi32Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi32Interval::xi_new(8, 10));
    }

    #[test]
    fn xi32_interval_tree_merge() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi32Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi32Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi32Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi32Interval::xi_new(10, 15));
    }

    #[test]
    fn xi32_interval_tree_all() {
        let mut tree = super::Xi32IntervalTree::xi_new();
        tree.xi_insert(super::Xi32Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi32Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi32_interval_tree_empty() {
        let tree = super::Xi32IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi32_interval_tree_contains_point() {
        let iv = super::Xi32Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 32) ---

    #[test]
    fn xj_32_uf_make_and_find() {
        let mut uf = super::Xj32UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_32_uf_union_connected() {
        let mut uf = super::Xj32UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_32_uf_component_count() {
        let mut uf = super::Xj32UnionFind::xj_new();
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
    fn xj_32_uf_component_size() {
        let mut uf = super::Xj32UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_32_uf_largest_component() {
        let mut uf = super::Xj32UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_32_uf_many_elements() {
        let mut uf = super::Xj32UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_32_uf_separate_components() {
        let mut uf = super::Xj32UnionFind::xj_new();
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
    fn xj_32_uf_path_compression() {
        let mut uf = super::Xj32UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_32_bt_insert_get() {
        let mut bt = super::Xj32BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_32_bt_contains_len() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_32_bt_replace() {
        let mut bt = super::Xj32BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_32_bt_remove() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_32_bt_keys_values() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_32_bt_range() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_32_bt_min_max() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_32_bt_many_inserts() {
        let mut bt = super::Xj32BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_32 segment tree tests ---

    #[test]
    fn xk_32_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_32_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk32SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_32_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_32_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_32_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_32_st_single_element() {
        let data = vec![42];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_32_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk32SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_32_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk32SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_32 disjoint intervals tests ---

    #[test]
    fn xk_32_di_add_and_count() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_32_di_merge_overlap() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_32_di_contains() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_32_di_remove() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_32_di_covered_length() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_32_di_gaps() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_32_di_merge_adjacent() {
        let mut di = super::Xk32DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_32_di_empty() {
        let di = super::Xk32DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_32_rope_new_empty() {
        let rope = super::Xl32Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_32_rope_from_str() {
        let rope = super::Xl32Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_32_rope_insert_at() {
        let mut rope = super::Xl32Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_32_rope_delete_range() {
        let mut rope = super::Xl32Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_32_rope_char_at() {
        let rope = super::Xl32Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_32_rope_split_concat() {
        let rope = super::Xl32Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_32_rope_line_count() {
        let rope = super::Xl32Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_32_rope_line_at() {
        let rope = super::Xl32Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_32_sa_build_and_search() {
        let sa = super::Xl32SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_32_sa_count() {
        let sa = super::Xl32SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_32_sa_longest_repeated() {
        let sa = super::Xl32SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_32_sa_all_positions() {
        let sa = super::Xl32SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_32_sa_len() {
        let sa = super::Xl32SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_32_sa_empty() {
        let sa = super::Xl32SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_32_rope_slice() {
        let rope = super::Xl32Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_32_sa_search_start() {
        let sa = super::Xl32SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_32_sparse_set_get() {
        let mut m = super::Xm32MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_32_sparse_row_col() {
        let mut m = super::Xm32MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_32_sparse_transpose() {
        let mut m = super::Xm32MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_32_sparse_multiply_vec() {
        let mut m = super::Xm32MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_32_sparse_nnz_density() {
        let mut m = super::Xm32MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_32_sparse_clear() {
        let mut m = super::Xm32MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_32_sparse_overwrite_zero() {
        let mut m = super::Xm32MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_32_tokenizer_basic() {
        let t = super::Xm32Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_32_tokenizer_count() {
        let t = super::Xm32Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_32_tokenizer_unique() {
        let t = super::Xm32Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_32_tokenizer_frequency() {
        let t = super::Xm32Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_32_tokenizer_delimiter() {
        let t = super::Xm32Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_32_tokenizer_whitespace() {
        let t = super::Xm32Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_32_tokenizer_empty() {
        let t = super::Xm32Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 32 ----

    #[test]
    fn xn_32_fenwick_prefix_sum() {
        let mut ft = super::Xn32Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_32_fenwick_range_sum() {
        let mut ft = super::Xn32Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_32_fenwick_point_query() {
        let mut ft = super::Xn32Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_32_fenwick_len() {
        let ft = super::Xn32Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_32_fenwick_multiple_updates() {
        let mut ft = super::Xn32Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_32_fenwick_single_element() {
        let mut ft = super::Xn32Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_32_fenwick_find_kth() {
        let mut ft = super::Xn32Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_32_fenwick_negative_delta() {
        let mut ft = super::Xn32Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 32 ----

    #[test]
    fn xn_32_avl_insert_get() {
        let mut m = super::Xn32AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_32_avl_remove() {
        let mut m = super::Xn32AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_32_avl_in_order() {
        let mut m = super::Xn32AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_32_avl_min_max() {
        let mut m = super::Xn32AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_32_avl_floor_ceiling() {
        let mut m = super::Xn32AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_32_avl_height_balanced() {
        let mut m = super::Xn32AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_32_avl_overwrite() {
        let mut m = super::Xn32AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_32_avl_empty() {
        let m: super::Xn32AVL<i32, i32> = super::Xn32AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
