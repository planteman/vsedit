//! Modal dialog system.

use std::fmt;

/// The kind of dialog to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Info,
    Warning,
    Error,
    Confirm,
}

impl fmt::Display for DialogKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogKind::Info => write!(f, "Info"),
            DialogKind::Warning => write!(f, "Warning"),
            DialogKind::Error => write!(f, "Error"),
            DialogKind::Confirm => write!(f, "Confirm"),
        }
    }
}

/// A button shown in a dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButton {
    pub label: String,
    pub is_primary: bool,
    pub returns_value: String,
}

impl DialogButton {
    pub fn new(label: impl Into<String>, returns_value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_primary: false,
            returns_value: returns_value.into(),
        }
    }

    pub fn primary(label: impl Into<String>, returns_value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_primary: true,
            returns_value: returns_value.into(),
        }
    }
}

/// Options for showing a dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogOptions {
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
    pub buttons: Vec<DialogButton>,
    pub detail: Option<String>,
}

impl DialogOptions {
    /// Convenience constructor for an OK / Cancel dialog.
    pub fn ok_cancel(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Info,
            buttons: vec![
                DialogButton::primary("OK", "ok"),
                DialogButton::new("Cancel", "cancel"),
            ],
            detail: None,
        }
    }

    /// Convenience constructor for a Yes / No / Cancel dialog.
    pub fn yes_no_cancel(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Confirm,
            buttons: vec![
                DialogButton::primary("Yes", "yes"),
                DialogButton::new("No", "no"),
                DialogButton::new("Cancel", "cancel"),
            ],
            detail: None,
        }
    }
}

/// The result of a dialog interaction.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogResult {
    pub button_value: Option<String>,
    pub cancelled: bool,
}

impl DialogResult {
    pub fn cancelled() -> Self {
        Self {
            button_value: None,
            cancelled: true,
        }
    }

    pub fn selected(value: impl Into<String>) -> Self {
        Self {
            button_value: Some(value.into()),
            cancelled: false,
        }
    }

    /// Returns `true` if the result has a button value matching `expected`.
    pub fn is_value(&self, expected: &str) -> bool {
        self.button_value.as_deref() == Some(expected)
    }
}

impl fmt::Display for DialogResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cancelled {
            write!(f, "Cancelled")
        } else if let Some(ref v) = self.button_value {
            write!(f, "Selected: {v}")
        } else {
            write!(f, "Cancelled")
        }
    }
}

/// Options for showing an input box.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBoxOptions {
    pub prompt: String,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    pub password: bool,
    pub validate_input: bool,
}

/// The result of an input box interaction.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBoxResult {
    pub value: Option<String>,
    pub cancelled: bool,
}

impl InputBoxResult {
    pub fn cancelled() -> Self {
        Self {
            value: None,
            cancelled: true,
        }
    }

    pub fn submitted(value: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            cancelled: false,
        }
    }
}

/// Trait for types that can show dialogs and input boxes.
pub trait DialogService {
    fn show_dialog(&self, options: DialogOptions) -> DialogResult;
    fn show_input_box(&self, options: InputBoxOptions) -> InputBoxResult;
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when working with dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogError {
    /// No buttons were provided in the dialog options.
    NoButtons,
    /// The user selected a value that is not among the dialog buttons.
    InvalidSelection(String),
    /// The dialog timed out waiting for user input.
    Timeout,
}

impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogError::NoButtons => write!(f, "dialog has no buttons"),
            DialogError::InvalidSelection(v) => write!(f, "invalid selection: {v}"),
            DialogError::Timeout => write!(f, "dialog timed out"),
        }
    }
}

impl std::error::Error for DialogError {}

// ---------------------------------------------------------------------------
// Builder patterns
// ---------------------------------------------------------------------------

/// Builder for constructing [`DialogOptions`] with chained methods.
#[derive(Debug, Clone)]
pub struct DialogOptionsBuilder {
    title: String,
    message: String,
    kind: DialogKind,
    buttons: Vec<DialogButton>,
    detail: Option<String>,
}

impl DialogOptionsBuilder {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Info,
            buttons: Vec::new(),
            detail: None,
        }
    }

    pub fn kind(mut self, kind: DialogKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn button(mut self, button: DialogButton) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn build(self) -> DialogOptions {
        DialogOptions {
            title: self.title,
            message: self.message,
            kind: self.kind,
            buttons: self.buttons,
            detail: self.detail,
        }
    }
}

/// Builder for constructing [`InputBoxOptions`] with chained methods.
#[derive(Debug, Clone)]
pub struct InputBoxOptionsBuilder {
    prompt: String,
    value: Option<String>,
    placeholder: Option<String>,
    password: bool,
    validate_input: bool,
}

impl InputBoxOptionsBuilder {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            value: None,
            placeholder: None,
            password: false,
            validate_input: false,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn validate_input(mut self, validate: bool) -> Self {
        self.validate_input = validate;
        self
    }

    pub fn build(self) -> InputBoxOptions {
        InputBoxOptions {
            prompt: self.prompt,
            value: self.value,
            placeholder: self.placeholder,
            password: self.password,
            validate_input: self.validate_input,
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience structs
// ---------------------------------------------------------------------------

/// Convenience struct for yes/no confirmation dialogs.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmDialogOptions {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
}

impl ConfirmDialogOptions {
    /// Create confirmation dialog options from a simple message.
    pub fn from_message(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            title: "Confirm".to_string(),
            message: msg,
            detail: None,
        }
    }

    /// Convert into full [`DialogOptions`] with Yes / No buttons.
    pub fn into_dialog_options(self) -> DialogOptions {
        DialogOptions {
            title: self.title,
            message: self.message,
            kind: DialogKind::Confirm,
            buttons: vec![
                DialogButton::primary("Yes", "yes"),
                DialogButton::new("No", "no"),
            ],
            detail: self.detail,
        }
    }
}

// ---------------------------------------------------------------------------
// In-memory / mock dialog service
// ---------------------------------------------------------------------------

/// A mock [`DialogService`] that returns pre-configured responses.
///
/// Useful for testing code that depends on user dialog interactions.
pub struct InMemoryDialogService {
    dialog_result: DialogResult,
    input_box_result: InputBoxResult,
}

impl InMemoryDialogService {
    pub fn new(dialog_result: DialogResult, input_box_result: InputBoxResult) -> Self {
        Self {
            dialog_result,
            input_box_result,
        }
    }

    /// Create a service that always returns a selected dialog value and a
    /// submitted input box value.
    pub fn always_ok() -> Self {
        Self {
            dialog_result: DialogResult::selected("ok"),
            input_box_result: InputBoxResult::submitted(""),
        }
    }

    /// Create a service that always returns cancelled results.
    pub fn always_cancel() -> Self {
        Self {
            dialog_result: DialogResult::cancelled(),
            input_box_result: InputBoxResult::cancelled(),
        }
    }
}

impl DialogService for InMemoryDialogService {
    fn show_dialog(&self, _options: DialogOptions) -> DialogResult {
        self.dialog_result.clone()
    }

    fn show_input_box(&self, _options: InputBoxOptions) -> InputBoxResult {
        self.input_box_result.clone()
    }
}

// ---------------------------------------------------------------------------
// Dialog history
// ---------------------------------------------------------------------------

/// Records shown dialogs and user responses for audit or replay.
#[derive(Debug, Clone)]
pub struct DialogHistoryEntry {
    pub dialog_id: String,
    pub kind: DialogKind,
    pub result: DialogResult,
}

/// Tracks a history of dialog interactions.
#[derive(Debug, Clone, Default)]
pub struct DialogHistory {
    entries: Vec<DialogHistoryEntry>,
}

impl DialogHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record that a dialog was shown and the user responded.
    pub fn record(&mut self, dialog_id: impl Into<String>, kind: DialogKind, result: DialogResult) {
        self.entries.push(DialogHistoryEntry {
            dialog_id: dialog_id.into(),
            kind,
            result,
        });
    }

    /// Get the last response for a given dialog id, if any.
    pub fn get_last_response(&self, dialog_id: &str) -> Option<&DialogResult> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.dialog_id == dialog_id)
            .map(|e| &e.result)
    }

    /// Total number of recorded interactions.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Remove all recorded history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Iterate over all entries.
    pub fn entries(&self) -> &[DialogHistoryEntry] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// Dialog validator
// ---------------------------------------------------------------------------

/// Errors produced by [`DialogValidator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    TooShort { min: usize, actual: usize },
    TooLong { max: usize, actual: usize },
    PatternMismatch { pattern: String },
    Empty,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::TooShort { min, actual } => {
                write!(f, "input too short: {actual} < {min}")
            }
            ValidationError::TooLong { max, actual } => {
                write!(f, "input too long: {actual} > {max}")
            }
            ValidationError::PatternMismatch { pattern } => {
                write!(f, "input does not match pattern: {pattern}")
            }
            ValidationError::Empty => write!(f, "input is empty"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validates user-supplied text against length and character-set constraints.
///
/// The `pattern` parameter in [`validate_input`](DialogValidator::validate_input)
/// is a simple character-class string (e.g. `"a-zA-Z0-9_"`) – **not** a full
/// regex – so that the crate avoids pulling in the `regex` dependency.
#[derive(Debug, Clone)]
pub struct DialogValidator;

impl DialogValidator {
    /// Validate `text` against the given constraints.
    ///
    /// * `min_len` / `max_len` – inclusive length bounds.
    /// * `pattern` – if `Some`, every character in `text` must appear in the
    ///   pattern string (simple allow-list).  Pass `None` to skip the check.
    pub fn validate_input(
        text: &str,
        min_len: usize,
        max_len: usize,
        pattern: Option<&str>,
    ) -> Result<(), ValidationError> {
        if text.is_empty() && min_len > 0 {
            return Err(ValidationError::Empty);
        }
        let len = text.len();
        if len < min_len {
            return Err(ValidationError::TooShort {
                min: min_len,
                actual: len,
            });
        }
        if len > max_len {
            return Err(ValidationError::TooLong {
                max: max_len,
                actual: len,
            });
        }
        if let Some(allowed) = pattern {
            for ch in text.chars() {
                if !allowed.contains(ch) {
                    return Err(ValidationError::PatternMismatch {
                        pattern: allowed.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dialog layout
// ---------------------------------------------------------------------------

/// Computes dialog dimensions based on content.
#[derive(Debug, Clone)]
pub struct DialogLayout {
    /// Minimum dialog width in columns.
    pub min_width: usize,
    /// Maximum dialog width in columns.
    pub max_width: usize,
    /// Horizontal padding on each side.
    pub padding_x: usize,
    /// Vertical padding (top + bottom).
    pub padding_y: usize,
    /// Height reserved for each button row.
    pub button_row_height: usize,
}

impl Default for DialogLayout {
    fn default() -> Self {
        Self {
            min_width: 30,
            max_width: 120,
            padding_x: 2,
            padding_y: 2,
            button_row_height: 3,
        }
    }
}

impl DialogLayout {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute the `(width, height)` of a dialog given its content.
    ///
    /// Width is determined by the longer of `title` and `message` (clamped to
    /// `min_width..=max_width`).  Height accounts for title, message lines,
    /// a button row, and padding.
    pub fn compute_size(&self, title: &str, message: &str, button_count: usize) -> (usize, usize) {
        let content_width = title.len().max(message.len()) + self.padding_x * 2;
        // Each button is roughly 10 columns wide plus spacing.
        let buttons_width = button_count * 10 + (button_count.saturating_sub(1)) * 2 + self.padding_x * 2;
        let raw_width = content_width.max(buttons_width);
        let width = raw_width.clamp(self.min_width, self.max_width);

        // Height: title (1) + blank line (1) + message lines + button row + padding
        let message_lines = if message.is_empty() {
            0
        } else {
            message.lines().count()
        };
        let height = 1 + 1 + message_lines + self.button_row_height + self.padding_y;

        (width, height)
    }
}

// ---------------------------------------------------------------------------
// Dialog theme
// ---------------------------------------------------------------------------

/// ANSI color code (foreground).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Format as a 24-bit ANSI foreground escape sequence.
    pub fn fg_ansi(&self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Format as a 24-bit ANSI background escape sequence.
    pub fn bg_ansi(&self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b)
    }
}

/// Border style for drawing dialog frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Single,
    Double,
    Rounded,
}

impl BorderStyle {
    /// Returns `(horizontal, vertical, top_left, top_right, bottom_left, bottom_right)`.
    pub fn chars(&self) -> (&str, &str, &str, &str, &str, &str) {
        match self {
            BorderStyle::None => (" ", " ", " ", " ", " ", " "),
            BorderStyle::Single => ("─", "│", "┌", "┐", "└", "┘"),
            BorderStyle::Double => ("═", "║", "╔", "╗", "╚", "╝"),
            BorderStyle::Rounded => ("─", "│", "╭", "╮", "╰", "╯"),
        }
    }
}

/// Visual theme for rendering dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogTheme {
    pub title_color: Color,
    pub message_color: Color,
    pub button_color: Color,
    pub button_primary_color: Color,
    pub background_color: Color,
    pub border_style: BorderStyle,
}

impl Default for DialogTheme {
    fn default() -> Self {
        Self {
            title_color: Color::rgb(255, 255, 255),
            message_color: Color::rgb(204, 204, 204),
            button_color: Color::rgb(180, 180, 180),
            button_primary_color: Color::rgb(100, 180, 255),
            background_color: Color::rgb(30, 30, 30),
            border_style: BorderStyle::Rounded,
        }
    }
}

impl DialogTheme {
    pub fn new() -> Self {
        Self::default()
    }

    /// A high-contrast theme suitable for accessibility.
    pub fn high_contrast() -> Self {
        Self {
            title_color: Color::rgb(255, 255, 0),
            message_color: Color::rgb(255, 255, 255),
            button_color: Color::rgb(255, 255, 255),
            button_primary_color: Color::rgb(0, 255, 0),
            background_color: Color::rgb(0, 0, 0),
            border_style: BorderStyle::Double,
        }
    }
}

// ---------------------------------------------------------------------------
// DialogButtonWithShortcut
// ---------------------------------------------------------------------------

/// A dialog button extended with an optional keyboard shortcut and tooltip.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButtonWithShortcut {
    pub button: DialogButton,
    pub shortcut: Option<char>,
    pub tooltip: Option<String>,
}

impl DialogButtonWithShortcut {
    pub fn new(button: DialogButton) -> Self {
        Self {
            button,
            shortcut: None,
            tooltip: None,
        }
    }

    pub fn with_shortcut(mut self, key: char) -> Self {
        self.shortcut = Some(key);
        self
    }

    pub fn with_tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }

    /// Returns `true` if `c` (case-insensitive) matches the shortcut key.
    pub fn matches_key(&self, c: char) -> bool {
        match self.shortcut {
            Some(k) => k.to_ascii_lowercase() == c.to_ascii_lowercase(),
            None => false,
        }
    }

    /// Returns a display label that indicates the shortcut key, e.g.
    /// label "OK" with shortcut 'O' → "OK [O]".
    pub fn display_label(&self) -> String {
        match self.shortcut {
            Some(k) => format!("{} [{}]", self.button.label, k.to_ascii_uppercase()),
            None => self.button.label.clone(),
        }
    }
}

impl fmt::Display for DialogButtonWithShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

// ---------------------------------------------------------------------------
// FileDialogFilter / FileDialogOptions
// ---------------------------------------------------------------------------

/// A single file-type filter entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileDialogFilter {
    pub fn new(name: impl Into<String>, extensions: Vec<String>) -> Self {
        Self {
            name: name.into(),
            extensions,
        }
    }

    /// Case-insensitive check whether `filename` matches one of the
    /// extensions in this filter.
    pub fn matches(&self, filename: &str) -> bool {
        let lower = filename.to_ascii_lowercase();
        self.extensions
            .iter()
            .any(|ext| lower.ends_with(&format!(".{}", ext.to_ascii_lowercase())))
    }

    /// Human-readable representation, e.g. "Rust files (*.rs)".
    pub fn display(&self) -> String {
        let exts: Vec<String> = self.extensions.iter().map(|e| format!("*.{e}")).collect();
        format!("{} ({})", self.name, exts.join(", "))
    }
}

impl fmt::Display for FileDialogFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Options for a file-open / file-save dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDialogOptions {
    pub title: String,
    pub filters: Vec<FileDialogFilter>,
    pub initial_dir: Option<String>,
}

impl FileDialogOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            filters: Vec::new(),
            initial_dir: None,
        }
    }

    /// Returns the first filter that matches `filename`, if any.
    pub fn find_matching_filter(&self, filename: &str) -> Option<&FileDialogFilter> {
        self.filters.iter().find(|f| f.matches(filename))
    }

    /// Collects every extension from all filters.
    pub fn all_extensions(&self) -> Vec<&str> {
        self.filters
            .iter()
            .flat_map(|f| f.extensions.iter().map(String::as_str))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DialogOutcome
// ---------------------------------------------------------------------------

/// A richer result type for dialog interactions.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogOutcome {
    Confirmed(String),
    Cancelled,
    TimedOut,
    Custom { key: String, data: Option<String> },
}

impl DialogOutcome {
    pub fn is_confirmed(&self) -> bool {
        matches!(self, DialogOutcome::Confirmed(_))
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, DialogOutcome::Cancelled)
    }

    /// Returns the inner value for `Confirmed` and `Custom` variants.
    pub fn value(&self) -> Option<&str> {
        match self {
            DialogOutcome::Confirmed(v) => Some(v.as_str()),
            DialogOutcome::Custom { data: Some(d), .. } => Some(d.as_str()),
            _ => None,
        }
    }
}

impl fmt::Display for DialogOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogOutcome::Confirmed(v) => write!(f, "Confirmed: {v}"),
            DialogOutcome::Cancelled => write!(f, "Cancelled"),
            DialogOutcome::TimedOut => write!(f, "Timed out"),
            DialogOutcome::Custom { key, data: Some(d) } => {
                write!(f, "Custom({key}): {d}")
            }
            DialogOutcome::Custom { key, data: None } => {
                write!(f, "Custom({key})")
            }
        }
    }
}


// ---------------------------------------------------------------------------
// DialogKind helpers
// ---------------------------------------------------------------------------

impl DialogKind {
    /// Returns all dialog kinds.
    pub fn all() -> &'static [DialogKind] {
        &[DialogKind::Info, DialogKind::Warning, DialogKind::Error, DialogKind::Confirm]
    }

    /// Returns an icon character for this dialog kind.
    pub fn icon(&self) -> char {
        match self {
            DialogKind::Info => 'ℹ',
            DialogKind::Warning => '⚠',
            DialogKind::Error => '✖',
            DialogKind::Confirm => '?',
        }
    }

    /// Returns true if this is an error or warning dialog.
    pub fn is_problem(&self) -> bool {
        matches!(self, DialogKind::Error | DialogKind::Warning)
    }

    /// Parse from a string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "info" | "information" => Some(Self::Info),
            "warning" | "warn" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "confirm" | "confirmation" => Some(Self::Confirm),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// DialogButton builder pattern
// ---------------------------------------------------------------------------

impl DialogButton {
    /// Set this button as primary.
    pub fn as_primary(mut self) -> Self {
        self.is_primary = true;
        self
    }

    /// Convenience constructor for an OK button.
    pub fn ok() -> Self {
        Self::new("OK", "ok").as_primary()
    }

    /// Convenience constructor for a Cancel button.
    pub fn cancel() -> Self {
        Self::new("Cancel", "cancel")
    }

    /// Convenience constructor for a Yes button.
    pub fn yes() -> Self {
        Self::new("Yes", "yes").as_primary()
    }

    /// Convenience constructor for a No button.
    pub fn no() -> Self {
        Self::new("No", "no")
    }
}

impl fmt::Display for DialogButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_primary {
            write!(f, "[{}]", self.label)
        } else {
            write!(f, " {} ", self.label)
        }
    }
}

// ---------------------------------------------------------------------------
// Dialog presets
// ---------------------------------------------------------------------------

/// Creates a standard OK dialog config.
pub fn info_dialog(title: &str, message: &str) -> DialogOptions {
    DialogOptions {
        kind: DialogKind::Info,
        title: title.to_string(),
        message: message.to_string(),
        buttons: vec![DialogButton::ok()],
        detail: None,
    }
}

/// Creates a confirm/cancel dialog config.
pub fn confirm_dialog(title: &str, message: &str) -> DialogOptions {
    DialogOptions {
        kind: DialogKind::Confirm,
        title: title.to_string(),
        message: message.to_string(),
        buttons: vec![DialogButton::yes(), DialogButton::no()],
        detail: None,
    }
}

/// Creates an error dialog config.
pub fn error_dialog(title: &str, message: &str) -> DialogOptions {
    DialogOptions {
        kind: DialogKind::Error,
        title: title.to_string(),
        message: message.to_string(),
        buttons: vec![DialogButton::ok()],
        detail: None,
    }
}

// ---------------------------------------------------------------------------
// FileDialogFilter helpers
// ---------------------------------------------------------------------------

/// Predefined filter for all files.
pub fn all_files_filter() -> FileDialogFilter {
    FileDialogFilter::new("All Files", vec!["*".to_string()])
}

/// Predefined filter for common image files.
pub fn image_filter() -> FileDialogFilter {
    FileDialogFilter::new("Images", vec!["png".into(), "jpg".into(), "jpeg".into(), "gif".into(), "bmp".into(), "svg".into()])
}

/// Predefined filter for text files.
pub fn text_filter() -> FileDialogFilter {
    FileDialogFilter::new("Text Files", vec!["txt".into(), "md".into(), "log".into()])
}

// ---------------------------------------------------------------------------
// InputDialog
// ---------------------------------------------------------------------------

/// An input dialog with optional validation.
#[derive(Debug, Clone)]
pub struct InputDialog {
    pub prompt: String,
    pub placeholder: String,
    pub max_length: usize,
    pub value: String,
}

impl InputDialog {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            placeholder: String::new(),
            max_length: 256,
            value: String::new(),
        }
    }

    pub fn with_placeholder(mut self, ph: impl Into<String>) -> Self {
        self.placeholder = ph.into();
        self
    }

    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = max;
        self
    }

    pub fn set_value(&mut self, val: impl Into<String>) {
        let val = val.into();
        if val.len() <= self.max_length {
            self.value = val;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.value.is_empty() {
            return Err("value cannot be empty".into());
        }
        if self.value.len() > self.max_length {
            return Err(format!("value exceeds max length {}", self.max_length));
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl fmt::Display for InputDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InputDialog(prompt='{}', max={})", self.prompt, self.max_length)
    }
}

// ---------------------------------------------------------------------------
// DialogStack
// ---------------------------------------------------------------------------

/// Manages a stack of open dialogs.
#[derive(Debug)]
pub struct DialogStack {
    stack: Vec<DialogOptions>,
}

impl DialogStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, dialog: DialogOptions) {
        self.stack.push(dialog);
    }

    pub fn pop(&mut self) -> Option<DialogOptions> {
        self.stack.pop()
    }

    pub fn peek(&self) -> Option<&DialogOptions> {
        self.stack.last()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Returns the titles of all dialogs in the stack (bottom-to-top).
    pub fn titles(&self) -> Vec<&str> {
        self.stack.iter().map(|d| d.title.as_str()).collect()
    }
}

impl Default for DialogStack {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DialogStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DialogStack({} dialogs)", self.stack.len())
    }
}

// ---------------------------------------------------------------------------
// Dialog utilities
// ---------------------------------------------------------------------------

/// Counts the number of primary buttons in a set of dialog options.
pub fn count_primary_buttons(options: &DialogOptions) -> usize {
    options.buttons.iter().filter(|b| b.is_primary).count()
}

/// Returns the labels of all buttons in a dialog options struct.
pub fn button_labels(options: &DialogOptions) -> Vec<&str> {
    options.buttons.iter().map(|b| b.label.as_str()).collect()
}

/// Returns `true` if the dialog options have a button whose `returns_value`
/// matches `value`.
pub fn has_button_value(options: &DialogOptions, value: &str) -> bool {
    options
        .buttons
        .iter()
        .any(|b| b.returns_value == value)
}

/// Returns the return-value of the first primary button, if any.
pub fn primary_button_value(options: &DialogOptions) -> Option<&str> {
    options
        .buttons
        .iter()
        .find(|b| b.is_primary)
        .map(|b| b.returns_value.as_str())
}

/// Builds a simple warning dialog with a single "OK" button.
pub fn warning_ok(title: impl Into<String>, message: impl Into<String>) -> DialogOptions {
    DialogOptions {
        title: title.into(),
        message: message.into(),
        kind: DialogKind::Warning,
        buttons: vec![DialogButton::primary("OK", "ok")],
        detail: None,
    }
}

/// Builds a simple error dialog with a single "Close" button.
pub fn error_close(title: impl Into<String>, message: impl Into<String>) -> DialogOptions {
    DialogOptions {
        title: title.into(),
        message: message.into(),
        kind: DialogKind::Error,
        buttons: vec![DialogButton::primary("Close", "close")],
        detail: None,
    }
}

/// Summarises a `DialogHistory` by counting how many dialogs of each kind
/// have been recorded.
pub fn history_kind_counts(history: &DialogHistory) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for entry in history.entries() {
        *map.entry(format!("{}", entry.kind)).or_insert(0) += 1;
    }
    map
}

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// DialogOptions helpers
// ---------------------------------------------------------------------------

/// Return `true` if the dialog has at least one primary button.
pub fn has_primary_button(options: &DialogOptions) -> bool {
    options.buttons.iter().any(|b| b.is_primary)
}

/// Return the number of buttons in a dialog.
pub fn dialog_button_count(options: &DialogOptions) -> usize {
    options.buttons.len()
}

/// Return `true` if this is a destructive dialog (Warning or Error kind).
pub fn is_destructive_dialog(options: &DialogOptions) -> bool {
    matches!(options.kind, DialogKind::Warning | DialogKind::Error)
}

/// Collect all button return values from dialog options.
pub fn dialog_return_values(options: &DialogOptions) -> Vec<&str> {
    options.buttons.iter().map(|b| b.returns_value.as_str()).collect()
}

/// Create a simple dialog with a single "OK" button.
pub fn simple_ok_dialog(title: impl Into<String>, message: impl Into<String>) -> DialogOptions {
    DialogOptions {
        title: title.into(),
        message: message.into(),
        kind: DialogKind::Info,
        buttons: vec![DialogButton::primary("OK", "ok")],
        detail: None,
    }
}

/// Create a save/discard/cancel dialog for unsaved changes.
pub fn save_discard_cancel(title: impl Into<String>, message: impl Into<String>) -> DialogOptions {
    DialogOptions {
        title: title.into(),
        message: message.into(),
        kind: DialogKind::Confirm,
        buttons: vec![
            DialogButton::primary("Save", "save"),
            DialogButton::new("Don't Save", "discard"),
            DialogButton::new("Cancel", "cancel"),
        ],
        detail: None,
    }
}

/// Return `true` if the dialog result indicates the user chose "ok".
pub fn is_ok_result(result: &DialogResult) -> bool {
    result.is_value("ok")
}

/// Return `true` if the dialog result indicates the user chose "cancel".
pub fn is_cancel_result(result: &DialogResult) -> bool {
    result.cancelled || result.is_value("cancel")
}

/// Count the total number of dialogs that were cancelled in a history.
pub fn cancelled_dialog_count(history: &DialogHistory) -> usize {
    history.entries().iter().filter(|e| e.result.cancelled).count()
}

/// Return the most recent dialog entry from a history, if any.
pub fn most_recent_dialog(history: &DialogHistory) -> Option<&DialogHistoryEntry> {
    history.entries().last()
}

/// Filter dialog history to only entries of a given kind.
pub fn history_filter_by_kind(history: &DialogHistory, kind: DialogKind) -> Vec<&DialogHistoryEntry> {
    history.entries().iter().filter(|e| e.kind == kind).collect()
}

// ---------------------------------------------------------------------------
// DialogButtonLayout
// ---------------------------------------------------------------------------

/// Horizontal alignment for buttons within a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAlignment {
    Left,
    Center,
    Right,
}

impl fmt::Display for ButtonAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ButtonAlignment::Left => write!(f, "left"),
            ButtonAlignment::Center => write!(f, "center"),
            ButtonAlignment::Right => write!(f, "right"),
        }
    }
}

/// Manages button positioning within a dialog: alignment, spacing, and
/// primary-button emphasis.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButtonLayout {
    pub alignment: ButtonAlignment,
    /// Spacing between adjacent buttons (in columns).
    pub spacing: usize,
    /// When `true`, the primary button is rendered wider to draw attention.
    pub emphasize_primary: bool,
    buttons: Vec<DialogButton>,
}

impl DialogButtonLayout {
    pub fn new(alignment: ButtonAlignment) -> Self {
        Self {
            alignment,
            spacing: 2,
            emphasize_primary: true,
            buttons: Vec::new(),
        }
    }

    pub fn with_spacing(mut self, spacing: usize) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn with_emphasis(mut self, emphasize: bool) -> Self {
        self.emphasize_primary = emphasize;
        self
    }

    pub fn add_button(&mut self, button: DialogButton) {
        self.buttons.push(button);
    }

    pub fn buttons(&self) -> &[DialogButton] {
        &self.buttons
    }

    pub fn primary_index(&self) -> Option<usize> {
        self.buttons.iter().position(|b| b.is_primary)
    }

    /// Compute the total width consumed by all buttons including spacing.
    pub fn total_width(&self) -> usize {
        if self.buttons.is_empty() {
            return 0;
        }
        let label_widths: usize = self.buttons.iter().map(|b| {
            let base = b.label.len() + 4; // padding around label
            if self.emphasize_primary && b.is_primary {
                base + 2 // extra emphasis width
            } else {
                base
            }
        }).sum();
        label_widths + self.spacing * self.buttons.len().saturating_sub(1)
    }

    /// Compute the left offset to apply given a container `width`.
    pub fn left_offset(&self, container_width: usize) -> usize {
        let tw = self.total_width();
        match self.alignment {
            ButtonAlignment::Left => 0,
            ButtonAlignment::Center => container_width.saturating_sub(tw) / 2,
            ButtonAlignment::Right => container_width.saturating_sub(tw),
        }
    }
}

impl fmt::Display for DialogButtonLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ButtonLayout({}, {} buttons, spacing={})",
            self.alignment,
            self.buttons.len(),
            self.spacing,
        )
    }
}

// ---------------------------------------------------------------------------
// DialogFormFields
// ---------------------------------------------------------------------------

/// The kind of input a form field expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormFieldKind {
    Text,
    Checkbox,
    Dropdown(Vec<String>),
}

/// A single field within a [`DialogFormFields`] form.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    pub name: String,
    pub label: String,
    pub kind: FormFieldKind,
    pub required: bool,
    pub value: String,
}

impl FormField {
    pub fn text(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind: FormFieldKind::Text,
            required: false,
            value: String::new(),
        }
    }

    pub fn checkbox(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind: FormFieldKind::Checkbox,
            required: false,
            value: "false".to_string(),
        }
    }

    pub fn dropdown(
        name: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        let initial = options.first().cloned().unwrap_or_default();
        Self {
            name: name.into(),
            label: label.into(),
            kind: FormFieldKind::Dropdown(options),
            required: false,
            value: initial,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn is_checked(&self) -> bool {
        matches!(self.kind, FormFieldKind::Checkbox) && self.value == "true"
    }

    pub fn set_checked(&mut self, checked: bool) {
        if matches!(self.kind, FormFieldKind::Checkbox) {
            self.value = if checked { "true" } else { "false" }.to_string();
        }
    }
}

/// Multi-field form within a dialog with per-field validation.
#[derive(Debug, Clone)]
pub struct DialogFormFields {
    fields: Vec<FormField>,
    /// Index of the currently focused field.
    focus_index: usize,
}

impl DialogFormFields {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            focus_index: 0,
        }
    }

    pub fn add_field(&mut self, field: FormField) {
        self.fields.push(field);
    }

    pub fn fields(&self) -> &[FormField] {
        &self.fields
    }

    pub fn fields_mut(&mut self) -> &mut [FormField] {
        &mut self.fields
    }

    pub fn field_by_name(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn field_by_name_mut(&mut self, name: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.name == name)
    }

    pub fn set_value(&mut self, name: &str, value: impl Into<String>) -> bool {
        if let Some(field) = self.field_by_name_mut(name) {
            field.value = value.into();
            true
        } else {
            false
        }
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    pub fn focus_next(&mut self) {
        if !self.fields.is_empty() {
            self.focus_index = (self.focus_index + 1) % self.fields.len();
        }
    }

    pub fn focus_prev(&mut self) {
        if !self.fields.is_empty() {
            self.focus_index = if self.focus_index == 0 {
                self.fields.len() - 1
            } else {
                self.focus_index - 1
            };
        }
    }

    /// Validate all fields. Returns a map of field name → error message for
    /// each field that fails validation.
    pub fn validate(&self) -> HashMap<String, String> {
        let mut errors = HashMap::new();
        for field in &self.fields {
            if field.required && field.value.is_empty() {
                errors.insert(
                    field.name.clone(),
                    format!("{} is required", field.label),
                );
            }
            if let FormFieldKind::Dropdown(ref opts) = field.kind {
                if !field.value.is_empty() && !opts.contains(&field.value) {
                    errors.insert(
                        field.name.clone(),
                        format!("'{}' is not a valid option", field.value),
                    );
                }
            }
        }
        errors
    }

    /// Returns `true` when all fields pass validation.
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }

    /// Collect all field values into a `HashMap`.
    pub fn values(&self) -> HashMap<String, String> {
        self.fields.iter().map(|f| (f.name.clone(), f.value.clone())).collect()
    }
}

impl Default for DialogFormFields {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DialogFormFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Form({} fields, focus={})", self.fields.len(), self.focus_index)
    }
}

// ---------------------------------------------------------------------------
// DialogProgressIndicator
// ---------------------------------------------------------------------------

/// Mode of the progress indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    /// A known percentage (0–100).
    Determinate,
    /// An unknown amount of work; show a spinner instead of a bar.
    Indeterminate,
}

/// Shows progress within a dialog for async operations.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogProgressIndicator {
    pub mode: ProgressMode,
    /// Current percentage (0–100). Only meaningful in `Determinate` mode.
    percentage: u8,
    pub message: String,
    pub completed: bool,
}

impl DialogProgressIndicator {
    pub fn determinate(message: impl Into<String>) -> Self {
        Self {
            mode: ProgressMode::Determinate,
            percentage: 0,
            message: message.into(),
            completed: false,
        }
    }

    pub fn indeterminate(message: impl Into<String>) -> Self {
        Self {
            mode: ProgressMode::Indeterminate,
            percentage: 0,
            message: message.into(),
            completed: false,
        }
    }

    pub fn percentage(&self) -> u8 {
        self.percentage
    }

    pub fn set_percentage(&mut self, pct: u8) {
        self.percentage = pct.min(100);
        if self.percentage == 100 {
            self.completed = true;
        }
    }

    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
    }

    pub fn finish(&mut self) {
        self.percentage = 100;
        self.completed = true;
    }

    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// Render a simple text progress bar, e.g. `[████░░░░░░] 40%`.
    pub fn render_bar(&self, width: usize) -> String {
        match self.mode {
            ProgressMode::Indeterminate => {
                format!("[{}] ...", "~".repeat(width))
            }
            ProgressMode::Determinate => {
                let filled = (self.percentage as usize * width) / 100;
                let empty = width.saturating_sub(filled);
                format!(
                    "[{}{}] {}%",
                    "█".repeat(filled),
                    "░".repeat(empty),
                    self.percentage,
                )
            }
        }
    }
}

impl fmt::Display for DialogProgressIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} – {}", self.render_bar(20), self.message)
    }
}

// ---------------------------------------------------------------------------
// DialogKeyboardShortcuts
// ---------------------------------------------------------------------------

/// An action that a keyboard shortcut can trigger within a dialog.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DialogAction {
    Confirm,
    Cancel,
    NextField,
    PrevField,
    /// Activate button at this index.
    SelectButton(usize),
    /// A custom named action.
    Custom(String),
}

impl fmt::Display for DialogAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogAction::Confirm => write!(f, "Confirm"),
            DialogAction::Cancel => write!(f, "Cancel"),
            DialogAction::NextField => write!(f, "Next Field"),
            DialogAction::PrevField => write!(f, "Previous Field"),
            DialogAction::SelectButton(i) => write!(f, "Select Button {i}"),
            DialogAction::Custom(name) => write!(f, "Custom({name})"),
        }
    }
}

/// A keyboard event identifier (simplified).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyEvent {
    Char(char),
    Enter,
    Escape,
    Tab,
    BackTab,
    F(u8),
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyEvent::Char(c) => write!(f, "'{c}'"),
            KeyEvent::Enter => write!(f, "Enter"),
            KeyEvent::Escape => write!(f, "Escape"),
            KeyEvent::Tab => write!(f, "Tab"),
            KeyEvent::BackTab => write!(f, "Shift+Tab"),
            KeyEvent::F(n) => write!(f, "F{n}"),
        }
    }
}

/// Maps keyboard events to dialog actions.
#[derive(Debug, Clone)]
pub struct DialogKeyboardShortcuts {
    bindings: Vec<(KeyEvent, DialogAction)>,
}

impl DialogKeyboardShortcuts {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Create a shortcut map with sensible defaults:
    /// Enter → Confirm, Escape → Cancel, Tab → NextField, BackTab → PrevField.
    pub fn with_defaults() -> Self {
        let mut s = Self::new();
        s.bind(KeyEvent::Enter, DialogAction::Confirm);
        s.bind(KeyEvent::Escape, DialogAction::Cancel);
        s.bind(KeyEvent::Tab, DialogAction::NextField);
        s.bind(KeyEvent::BackTab, DialogAction::PrevField);
        s
    }

    pub fn bind(&mut self, key: KeyEvent, action: DialogAction) {
        // Replace existing binding for the same key.
        if let Some(existing) = self.bindings.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = action;
        } else {
            self.bindings.push((key, action));
        }
    }

    pub fn unbind(&mut self, key: &KeyEvent) {
        self.bindings.retain(|(k, _)| k != key);
    }

    pub fn lookup(&self, key: &KeyEvent) -> Option<&DialogAction> {
        self.bindings.iter().find(|(k, _)| k == key).map(|(_, a)| a)
    }

    pub fn bindings(&self) -> &[(KeyEvent, DialogAction)] {
        &self.bindings
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Return all keys that map to a given action.
    pub fn keys_for_action(&self, action: &DialogAction) -> Vec<&KeyEvent> {
        self.bindings
            .iter()
            .filter(|(_, a)| a == action)
            .map(|(k, _)| k)
            .collect()
    }
}

impl Default for DialogKeyboardShortcuts {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl fmt::Display for DialogKeyboardShortcuts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shortcuts({} bindings)", self.bindings.len())
    }
}

// ---------------------------------------------------------------------------
// DialogLayoutCalculator — compute dialog position relative to parent
// ---------------------------------------------------------------------------

/// Computes dialog position and size relative to a parent rectangle.
#[derive(Debug, Clone)]
pub struct DialogLayoutCalculator {
    pub parent_width: usize,
    pub parent_height: usize,
    pub margin: usize,
}

impl DialogLayoutCalculator {
    pub fn new(parent_width: usize, parent_height: usize) -> Self {
        Self { parent_width, parent_height, margin: 2 }
    }

    pub fn with_margin(mut self, margin: usize) -> Self {
        self.margin = margin;
        self
    }

    /// Center a dialog of given `(w, h)` inside the parent.
    /// Returns `(x, y)`.
    pub fn center(&self, dialog_width: usize, dialog_height: usize) -> (usize, usize) {
        let x = self.parent_width.saturating_sub(dialog_width) / 2;
        let y = self.parent_height.saturating_sub(dialog_height) / 2;
        (x, y)
    }

    /// Clamp a dialog size to fit within the parent minus margins.
    pub fn clamp_size(&self, desired_width: usize, desired_height: usize) -> (usize, usize) {
        let max_w = self.parent_width.saturating_sub(self.margin * 2);
        let max_h = self.parent_height.saturating_sub(self.margin * 2);
        (desired_width.min(max_w), desired_height.min(max_h))
    }

    /// Compute the full rect (x, y, w, h) for a centred, clamped dialog.
    pub fn compute_rect(&self, desired_width: usize, desired_height: usize) -> (usize, usize, usize, usize) {
        let (w, h) = self.clamp_size(desired_width, desired_height);
        let (x, y) = self.center(w, h);
        (x, y, w, h)
    }

    /// Available width after margins.
    pub fn available_width(&self) -> usize {
        self.parent_width.saturating_sub(self.margin * 2)
    }

    /// Available height after margins.
    pub fn available_height(&self) -> usize {
        self.parent_height.saturating_sub(self.margin * 2)
    }
}

// ---------------------------------------------------------------------------
// DialogAnimationState — opening/closing transition tracking
// ---------------------------------------------------------------------------

/// Tracks the animation state of a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAnimationPhase {
    Opening,
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Clone)]
pub struct DialogAnimationState {
    phase: DialogAnimationPhase,
    progress: f64,
    duration_ms: u64,
    elapsed_ms: u64,
}

impl DialogAnimationState {
    pub fn new(duration_ms: u64) -> Self {
        Self { phase: DialogAnimationPhase::Closed, progress: 0.0, duration_ms, elapsed_ms: 0 }
    }

    pub fn start_opening(&mut self) {
        self.phase = DialogAnimationPhase::Opening;
        self.elapsed_ms = 0;
        self.progress = 0.0;
    }

    pub fn start_closing(&mut self) {
        self.phase = DialogAnimationPhase::Closing;
        self.elapsed_ms = 0;
        self.progress = 1.0;
    }

    /// Advance the animation by `delta_ms`. Returns `true` if the phase just completed.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        self.elapsed_ms += delta_ms;
        let frac = if self.duration_ms == 0 {
            1.0
        } else {
            (self.elapsed_ms as f64 / self.duration_ms as f64).min(1.0)
        };

        match self.phase {
            DialogAnimationPhase::Opening => {
                self.progress = frac;
                if frac >= 1.0 { self.phase = DialogAnimationPhase::Open; return true; }
            }
            DialogAnimationPhase::Closing => {
                self.progress = 1.0 - frac;
                if frac >= 1.0 { self.phase = DialogAnimationPhase::Closed; return true; }
            }
            _ => {}
        }
        false
    }

    pub fn phase(&self) -> DialogAnimationPhase { self.phase }
    pub fn progress(&self) -> f64 { self.progress }
    pub fn is_visible(&self) -> bool { self.phase != DialogAnimationPhase::Closed }
    pub fn is_animating(&self) -> bool {
        matches!(self.phase, DialogAnimationPhase::Opening | DialogAnimationPhase::Closing)
    }
}

// ---------------------------------------------------------------------------
// DialogFocusTrap — trap focus within a dialog
// ---------------------------------------------------------------------------

/// Traps keyboard focus within a dialog, cycling through focusable elements.
#[derive(Debug, Clone)]
pub struct DialogFocusTrap {
    elements: Vec<String>,
    focused_index: Option<usize>,
    previous_focus: Option<String>,
}

impl DialogFocusTrap {
    pub fn new() -> Self {
        Self { elements: Vec::new(), focused_index: None, previous_focus: None }
    }

    /// Set the element that was focused before the dialog opened.
    pub fn set_previous_focus(&mut self, id: impl Into<String>) {
        self.previous_focus = Some(id.into());
    }

    /// Register a focusable element.
    pub fn add_element(&mut self, id: impl Into<String>) {
        self.elements.push(id.into());
    }

    /// Focus the first element.
    pub fn focus_first(&mut self) -> Option<&str> {
        if self.elements.is_empty() { return None; }
        self.focused_index = Some(0);
        Some(&self.elements[0])
    }

    /// Cycle focus forward.
    pub fn cycle_next(&mut self) -> Option<&str> {
        if self.elements.is_empty() { return None; }
        let next = match self.focused_index {
            Some(i) => (i + 1) % self.elements.len(),
            None => 0,
        };
        self.focused_index = Some(next);
        Some(&self.elements[next])
    }

    /// Cycle focus backward.
    pub fn cycle_prev(&mut self) -> Option<&str> {
        if self.elements.is_empty() { return None; }
        let prev = match self.focused_index {
            Some(0) => self.elements.len() - 1,
            Some(i) => i - 1,
            None => self.elements.len() - 1,
        };
        self.focused_index = Some(prev);
        Some(&self.elements[prev])
    }

    /// The element ID to restore focus to after dialog closes.
    pub fn restore_target(&self) -> Option<&str> {
        self.previous_focus.as_deref()
    }

    pub fn current(&self) -> Option<&str> {
        self.focused_index.map(|i| self.elements[i].as_str())
    }

    pub fn len(&self) -> usize { self.elements.len() }
    pub fn is_empty(&self) -> bool { self.elements.is_empty() }
}

impl Default for DialogFocusTrap {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_result_cancelled() {
        let r = DialogResult::cancelled();
        assert!(r.cancelled);
        assert!(r.button_value.is_none());
    }

    #[test]
    fn dialog_result_selected() {
        let r = DialogResult::selected("ok");
        assert!(!r.cancelled);
        assert_eq!(r.button_value.as_deref(), Some("ok"));
    }

    #[test]
    fn dialog_button_primary() {
        let btn = DialogButton::primary("OK", "ok");
        assert!(btn.is_primary);
        assert_eq!(btn.label, "OK");
        assert_eq!(btn.returns_value, "ok");
    }

    #[test]
    fn input_box_result_submitted() {
        let r = InputBoxResult::submitted("hello");
        assert!(!r.cancelled);
        assert_eq!(r.value.as_deref(), Some("hello"));
    }

    // ----- new tests -----

    #[test]
    fn dialog_error_display_no_buttons() {
        let e = DialogError::NoButtons;
        assert_eq!(e.to_string(), "dialog has no buttons");
    }

    #[test]
    fn dialog_error_display_invalid_selection() {
        let e = DialogError::InvalidSelection("bad".into());
        assert_eq!(e.to_string(), "invalid selection: bad");
    }

    #[test]
    fn dialog_error_display_timeout() {
        assert_eq!(DialogError::Timeout.to_string(), "dialog timed out");
    }

    #[test]
    fn dialog_kind_display() {
        assert_eq!(DialogKind::Info.to_string(), "Info");
        assert_eq!(DialogKind::Warning.to_string(), "Warning");
        assert_eq!(DialogKind::Error.to_string(), "Error");
        assert_eq!(DialogKind::Confirm.to_string(), "Confirm");
    }

    #[test]
    fn dialog_result_display() {
        assert_eq!(DialogResult::selected("ok").to_string(), "Selected: ok");
        assert_eq!(DialogResult::cancelled().to_string(), "Cancelled");
    }

    #[test]
    fn dialog_result_is_value() {
        let r = DialogResult::selected("yes");
        assert!(r.is_value("yes"));
        assert!(!r.is_value("no"));
        assert!(!DialogResult::cancelled().is_value("yes"));
    }

    #[test]
    fn dialog_options_ok_cancel() {
        let opts = DialogOptions::ok_cancel("Title", "Msg");
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.buttons[0].returns_value, "ok");
        assert!(opts.buttons[0].is_primary);
        assert_eq!(opts.buttons[1].returns_value, "cancel");
        assert!(!opts.buttons[1].is_primary);
    }

    #[test]
    fn dialog_options_yes_no_cancel() {
        let opts = DialogOptions::yes_no_cancel("Save?", "Do you want to save?");
        assert_eq!(opts.kind, DialogKind::Confirm);
        assert_eq!(opts.buttons.len(), 3);
        assert_eq!(opts.buttons[0].returns_value, "yes");
        assert_eq!(opts.buttons[1].returns_value, "no");
        assert_eq!(opts.buttons[2].returns_value, "cancel");
    }

    #[test]
    fn dialog_options_builder() {
        let opts = DialogOptionsBuilder::new("Title", "Message")
            .kind(DialogKind::Warning)
            .button(DialogButton::primary("Save", "save"))
            .button(DialogButton::new("Discard", "discard"))
            .detail("Some detail")
            .build();
        assert_eq!(opts.title, "Title");
        assert_eq!(opts.kind, DialogKind::Warning);
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.detail.as_deref(), Some("Some detail"));
    }

    #[test]
    fn input_box_options_builder() {
        let opts = InputBoxOptionsBuilder::new("Enter name")
            .value("default")
            .placeholder("Type here…")
            .password(true)
            .validate_input(true)
            .build();
        assert_eq!(opts.prompt, "Enter name");
        assert_eq!(opts.value.as_deref(), Some("default"));
        assert_eq!(opts.placeholder.as_deref(), Some("Type here…"));
        assert!(opts.password);
        assert!(opts.validate_input);
    }

    #[test]
    fn in_memory_dialog_service_always_ok() {
        let svc = InMemoryDialogService::always_ok();
        let opts = DialogOptions::ok_cancel("T", "M");
        let result = svc.show_dialog(opts);
        assert!(result.is_value("ok"));
        assert!(!result.cancelled);
    }

    #[test]
    fn in_memory_dialog_service_always_cancel() {
        let svc = InMemoryDialogService::always_cancel();
        let opts = DialogOptions::ok_cancel("T", "M");
        let result = svc.show_dialog(opts);
        assert!(result.cancelled);

        let ib_opts = InputBoxOptionsBuilder::new("Prompt").build();
        let ib_result = svc.show_input_box(ib_opts);
        assert!(ib_result.cancelled);
    }

    #[test]
    fn confirm_dialog_options_from_message() {
        let confirm = ConfirmDialogOptions::from_message("Delete file?");
        assert_eq!(confirm.title, "Confirm");
        assert_eq!(confirm.message, "Delete file?");
        let opts = confirm.into_dialog_options();
        assert_eq!(opts.kind, DialogKind::Confirm);
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.buttons[0].returns_value, "yes");
        assert_eq!(opts.buttons[1].returns_value, "no");
    }

    // ----- DialogHistory tests -----

    #[test]
    fn dialog_history_record_and_query() {
        let mut history = DialogHistory::new();
        assert_eq!(history.count(), 0);

        history.record("save-confirm", DialogKind::Confirm, DialogResult::selected("yes"));
        history.record("delete-warn", DialogKind::Warning, DialogResult::selected("no"));
        assert_eq!(history.count(), 2);

        let last = history.get_last_response("save-confirm").unwrap();
        assert!(last.is_value("yes"));

        assert!(history.get_last_response("nonexistent").is_none());
    }

    #[test]
    fn dialog_history_last_response_returns_most_recent() {
        let mut history = DialogHistory::new();
        history.record("dlg", DialogKind::Info, DialogResult::selected("first"));
        history.record("dlg", DialogKind::Info, DialogResult::selected("second"));
        let last = history.get_last_response("dlg").unwrap();
        assert!(last.is_value("second"));
    }

    #[test]
    fn dialog_history_clear() {
        let mut history = DialogHistory::new();
        history.record("a", DialogKind::Info, DialogResult::cancelled());
        history.clear();
        assert_eq!(history.count(), 0);
        assert!(history.get_last_response("a").is_none());
    }

    // ----- DialogValidator tests -----

    #[test]
    fn validator_accepts_valid_input() {
        assert!(DialogValidator::validate_input("hello", 1, 10, None).is_ok());
    }

    #[test]
    fn validator_rejects_empty() {
        let err = DialogValidator::validate_input("", 1, 10, None).unwrap_err();
        assert_eq!(err, ValidationError::Empty);
        assert_eq!(err.to_string(), "input is empty");
    }

    #[test]
    fn validator_rejects_too_short() {
        let err = DialogValidator::validate_input("ab", 3, 10, None).unwrap_err();
        assert_eq!(err, ValidationError::TooShort { min: 3, actual: 2 });
    }

    #[test]
    fn validator_rejects_too_long() {
        let err = DialogValidator::validate_input("abcdef", 1, 3, None).unwrap_err();
        assert_eq!(err, ValidationError::TooLong { max: 3, actual: 6 });
    }

    #[test]
    fn validator_pattern_mismatch() {
        let err = DialogValidator::validate_input("abc!", 1, 10, Some("abc")).unwrap_err();
        match err {
            ValidationError::PatternMismatch { ref pattern } => assert_eq!(pattern, "abc"),
            other => panic!("expected PatternMismatch, got {other:?}"),
        }
    }

    #[test]
    fn validator_pattern_ok() {
        assert!(DialogValidator::validate_input("abc", 1, 10, Some("abcdef")).is_ok());
    }

    // ----- DialogLayout tests -----

    #[test]
    fn layout_compute_size_basic() {
        let layout = DialogLayout::new();
        let (w, h) = layout.compute_size("Title", "Hello world", 2);
        assert!(w >= layout.min_width);
        assert!(w <= layout.max_width);
        // height = 1 (title) + 1 (blank) + 1 (message line) + 3 (button row) + 2 (padding) = 8
        assert_eq!(h, 8);
    }

    #[test]
    fn layout_respects_min_width() {
        let layout = DialogLayout { min_width: 50, ..DialogLayout::default() };
        let (w, _) = layout.compute_size("Hi", "Ok", 1);
        assert!(w >= 50);
    }

    #[test]
    fn layout_multiline_message() {
        let layout = DialogLayout::new();
        let msg = "Line one\nLine two\nLine three";
        let (_, h) = layout.compute_size("T", msg, 1);
        // 1 + 1 + 3 + 3 + 2 = 10
        assert_eq!(h, 10);
    }

    // ----- DialogTheme tests -----

    #[test]
    fn theme_default_has_rounded_border() {
        let theme = DialogTheme::new();
        assert_eq!(theme.border_style, BorderStyle::Rounded);
    }

    #[test]
    fn theme_high_contrast() {
        let theme = DialogTheme::high_contrast();
        assert_eq!(theme.border_style, BorderStyle::Double);
        assert_eq!(theme.background_color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn color_ansi_sequences() {
        let c = Color::rgb(255, 128, 0);
        assert!(c.fg_ansi().contains("38;2;255;128;0"));
        assert!(c.bg_ansi().contains("48;2;255;128;0"));
    }

    #[test]
    fn border_style_chars() {
        let (h, v, tl, tr, bl, br) = BorderStyle::Rounded.chars();
        assert_eq!(h, "─");
        assert_eq!(v, "│");
        assert_eq!(tl, "╭");
        assert_eq!(tr, "╮");
        assert_eq!(bl, "╰");
        assert_eq!(br, "╯");
    }

    // -- DialogButtonWithShortcut tests --

    #[test]
    fn button_with_shortcut_display_label() {
        let btn = DialogButtonWithShortcut::new(DialogButton::new("OK", "ok"))
            .with_shortcut('o');
        assert_eq!(btn.display_label(), "OK [O]");
    }

    #[test]
    fn button_with_shortcut_no_shortcut() {
        let btn = DialogButtonWithShortcut::new(DialogButton::new("Cancel", "cancel"));
        assert_eq!(btn.display_label(), "Cancel");
        assert!(!btn.matches_key('c'));
    }

    #[test]
    fn button_with_shortcut_matches_case_insensitive() {
        let btn = DialogButtonWithShortcut::new(DialogButton::primary("Save", "save"))
            .with_shortcut('s')
            .with_tooltip("Save the file");
        assert!(btn.matches_key('s'));
        assert!(btn.matches_key('S'));
        assert!(!btn.matches_key('x'));
        assert_eq!(btn.tooltip.as_deref(), Some("Save the file"));
    }

    // -- FileDialogFilter tests --

    #[test]
    fn file_filter_matches_extension() {
        let f = FileDialogFilter::new("Rust files", vec!["rs".into()]);
        assert!(f.matches("main.rs"));
        assert!(f.matches("LIB.RS")); // case-insensitive
        assert!(!f.matches("main.py"));
        assert_eq!(f.display(), "Rust files (*.rs)");
    }

    #[test]
    fn file_filter_multiple_extensions() {
        let f = FileDialogFilter::new("Images", vec!["png".into(), "jpg".into(), "gif".into()]);
        assert!(f.matches("photo.JPG"));
        assert!(f.matches("icon.png"));
        assert!(!f.matches("doc.pdf"));
        assert_eq!(f.display(), "Images (*.png, *.jpg, *.gif)");
    }

    #[test]
    fn file_dialog_options_find_and_all() {
        let opts = FileDialogOptions {
            title: "Open".into(),
            filters: vec![
                FileDialogFilter::new("Rust", vec!["rs".into()]),
                FileDialogFilter::new("TOML", vec!["toml".into()]),
            ],
            initial_dir: Some("/home".into()),
        };
        assert_eq!(opts.find_matching_filter("Cargo.toml").unwrap().name, "TOML");
        assert!(opts.find_matching_filter("image.png").is_none());
        assert_eq!(opts.all_extensions(), vec!["rs", "toml"]);
    }

    // -- DialogOutcome tests --

    #[test]
    fn dialog_outcome_confirmed() {
        let o = DialogOutcome::Confirmed("yes".into());
        assert!(o.is_confirmed());
        assert!(!o.is_cancelled());
        assert_eq!(o.value(), Some("yes"));
        assert_eq!(o.to_string(), "Confirmed: yes");
    }

    #[test]
    fn dialog_outcome_variants() {
        assert!(DialogOutcome::Cancelled.is_cancelled());
        assert_eq!(DialogOutcome::Cancelled.value(), None);
        assert_eq!(DialogOutcome::TimedOut.to_string(), "Timed out");

        let custom = DialogOutcome::Custom {
            key: "retry".into(),
            data: Some("3".into()),
        };
        assert_eq!(custom.value(), Some("3"));
        assert_eq!(custom.to_string(), "Custom(retry): 3");

        let custom_no_data = DialogOutcome::Custom {
            key: "skip".into(),
            data: None,
        };
        assert_eq!(custom_no_data.value(), None);
        assert_eq!(custom_no_data.to_string(), "Custom(skip)");
    }

    #[test]
    fn test_dialog_kind_all() {
        assert_eq!(DialogKind::all().len(), 4);
    }

    #[test]
    fn test_dialog_kind_icon() {
        assert_eq!(DialogKind::Info.icon(), 'ℹ');
        assert_eq!(DialogKind::Error.icon(), '✖');
    }

    #[test]
    fn test_dialog_kind_is_problem() {
        assert!(DialogKind::Error.is_problem());
        assert!(DialogKind::Warning.is_problem());
        assert!(!DialogKind::Info.is_problem());
    }

    #[test]
    fn test_dialog_kind_from_name() {
        assert_eq!(DialogKind::from_name("info"), Some(DialogKind::Info));
        assert_eq!(DialogKind::from_name("WARN"), Some(DialogKind::Warning));
        assert_eq!(DialogKind::from_name("nope"), None);
    }

    #[test]
    fn test_dialog_button_presets() {
        let ok = DialogButton::ok();
        assert!(ok.is_primary);
        assert_eq!(ok.returns_value, "ok");
        let cancel = DialogButton::cancel();
        assert!(!cancel.is_primary);
    }

    #[test]
    fn test_dialog_button_display() {
        let ok = DialogButton::ok();
        assert_eq!(format!("{ok}"), "[OK]");
        let cancel = DialogButton::cancel();
        assert_eq!(format!("{cancel}"), " Cancel ");
    }

    #[test]
    fn test_info_dialog_preset() {
        let d = info_dialog("Title", "Message");
        assert_eq!(d.kind, DialogKind::Info);
        assert_eq!(d.buttons.len(), 1);
        assert!(d.buttons[0].is_primary);
    }

    #[test]
    fn test_confirm_dialog_preset() {
        let d = confirm_dialog("Delete?", "Are you sure?");
        assert_eq!(d.kind, DialogKind::Confirm);
        assert_eq!(d.buttons.len(), 2);
    }

    #[test]
    fn test_error_dialog_preset() {
        let d = error_dialog("Error", "Something failed");
        assert_eq!(d.kind, DialogKind::Error);
    }

    #[test]
    fn test_file_filters() {
        let all = all_files_filter();
        assert!(format!("{all}").contains("All Files"));
        let img = image_filter();
        assert!(img.extensions.contains(&"png".to_string()));
        let txt = text_filter();
        assert!(txt.extensions.contains(&"txt".to_string()));
    }

    #[test]
    fn test_dialog_theme_high_contrast() {
        let theme = DialogTheme::high_contrast();
        assert_eq!(theme.border_style, BorderStyle::Double);
        assert_eq!(theme.background_color, Color::rgb(0, 0, 0));
        let default_theme = DialogTheme::default();
        assert_eq!(default_theme.border_style, BorderStyle::Rounded);
    }

    #[test]
    fn test_input_dialog_validation() {
        let mut dlg = InputDialog::new("Enter name")
            .with_placeholder("e.g. John")
            .with_max_length(10);
        assert!(dlg.validate().is_err()); // empty
        dlg.set_value("John");
        assert!(dlg.validate().is_ok());
        dlg.set_value("VeryLongNameExceeding");
        assert!(dlg.is_empty() || dlg.value == "John"); // rejected due to max_length
        assert!(format!("{dlg}").contains("Enter name"));
    }

    #[test]
    fn test_input_dialog_max_length_enforcement() {
        let mut dlg = InputDialog::new("test").with_max_length(3);
        dlg.set_value("ab");
        assert_eq!(dlg.value, "ab");
        dlg.set_value("abcd");
        assert_eq!(dlg.value, "ab"); // unchanged, too long
    }

    #[test]
    fn test_dialog_stack_operations() {
        let mut stack = DialogStack::new();
        assert!(stack.is_empty());
        stack.push(DialogOptions::ok_cancel("First", "Message 1"));
        stack.push(DialogOptions::ok_cancel("Second", "Message 2"));
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.peek().unwrap().title, "Second");
        assert_eq!(stack.titles(), vec!["First", "Second"]);
        let popped = stack.pop().unwrap();
        assert_eq!(popped.title, "Second");
        assert_eq!(stack.len(), 1);
        assert!(format!("{stack}").contains("1 dialogs"));
    }

    #[test]
    fn test_dialog_stack_clear() {
        let mut stack = DialogStack::new();
        stack.push(info_dialog("A", "a"));
        stack.push(info_dialog("B", "b"));
        stack.clear();
        assert!(stack.is_empty());
        assert!(stack.pop().is_none());
    }

    #[test]
    fn test_dialog_stack_peek_empty() {
        let stack = DialogStack::new();
        assert!(stack.peek().is_none());
    }

    // --- new tests ---

    #[test]
    fn test_count_primary_buttons_none() {
        let opts = DialogOptions {
            title: "T".into(),
            message: "M".into(),
            kind: DialogKind::Info,
            buttons: vec![DialogButton::new("A", "a"), DialogButton::new("B", "b")],
            detail: None,
        };
        assert_eq!(count_primary_buttons(&opts), 0);
    }

    #[test]
    fn test_count_primary_buttons_some() {
        let opts = DialogOptions::ok_cancel("T", "M");
        assert_eq!(count_primary_buttons(&opts), 1);
    }

    #[test]
    fn test_button_labels_ok_cancel() {
        let opts = DialogOptions::ok_cancel("T", "M");
        assert_eq!(button_labels(&opts), vec!["OK", "Cancel"]);
    }

    #[test]
    fn test_has_button_value_present() {
        let opts = DialogOptions::ok_cancel("T", "M");
        assert!(has_button_value(&opts, "ok"));
        assert!(has_button_value(&opts, "cancel"));
        assert!(!has_button_value(&opts, "maybe"));
    }

    #[test]
    fn test_primary_button_value_found() {
        let opts = DialogOptions::ok_cancel("T", "M");
        assert_eq!(primary_button_value(&opts), Some("ok"));
    }

    #[test]
    fn test_primary_button_value_none() {
        let opts = DialogOptions {
            title: "T".into(),
            message: "M".into(),
            kind: DialogKind::Info,
            buttons: vec![DialogButton::new("X", "x")],
            detail: None,
        };
        assert_eq!(primary_button_value(&opts), None);
    }

    #[test]
    fn test_warning_ok_dialog() {
        let opts = warning_ok("Warn", "Something happened");
        assert_eq!(opts.kind, DialogKind::Warning);
        assert_eq!(opts.buttons.len(), 1);
        assert!(opts.buttons[0].is_primary);
    }

    #[test]
    fn test_error_close_dialog() {
        let opts = error_close("Err", "Fatal");
        assert_eq!(opts.kind, DialogKind::Error);
        assert_eq!(opts.buttons[0].returns_value, "close");
    }

    #[test]
    fn test_history_kind_counts_empty() {
        let history = DialogHistory::new();
        let counts = history_kind_counts(&history);
        assert!(counts.is_empty());
    }

    #[test]
    fn test_history_kind_counts_mixed() {
        let mut history = DialogHistory::new();
        history.record("a", DialogKind::Info, DialogResult::cancelled());
        history.record("b", DialogKind::Info, DialogResult::selected("ok"));
        history.record("c", DialogKind::Error, DialogResult::cancelled());
        let counts = history_kind_counts(&history);
        assert_eq!(counts.get("Info"), Some(&2));
        assert_eq!(counts.get("Error"), Some(&1));
    }

    #[test]
    fn has_primary_button_true() {
        let opts = DialogOptions::ok_cancel("Title", "Msg");
        assert!(has_primary_button(&opts));
    }

    #[test]
    fn has_primary_button_false() {
        let opts = DialogOptions {
            title: "T".into(),
            message: "M".into(),
            kind: DialogKind::Info,
            buttons: vec![DialogButton::new("A", "a")],
            detail: None,
        };
        assert!(!has_primary_button(&opts));
    }

    #[test]
    fn dialog_button_count_works() {
        let opts = DialogOptions::yes_no_cancel("T", "M");
        assert_eq!(dialog_button_count(&opts), 3);
    }

    #[test]
    fn is_destructive_dialog_checks_kind() {
        let warn = DialogOptions {
            title: "T".into(),
            message: "M".into(),
            kind: DialogKind::Warning,
            buttons: vec![],
            detail: None,
        };
        assert!(is_destructive_dialog(&warn));
        let info = DialogOptions {
            title: "T".into(),
            message: "M".into(),
            kind: DialogKind::Info,
            buttons: vec![],
            detail: None,
        };
        assert!(!is_destructive_dialog(&info));
    }

    #[test]
    fn dialog_return_values_collected() {
        let opts = DialogOptions::ok_cancel("T", "M");
        let vals = dialog_return_values(&opts);
        assert!(vals.contains(&"ok"));
        assert!(vals.contains(&"cancel"));
    }

    #[test]
    fn simple_ok_dialog_has_one_button() {
        let opts = simple_ok_dialog("Hello", "World");
        assert_eq!(opts.buttons.len(), 1);
        assert!(opts.buttons[0].is_primary);
        assert_eq!(opts.buttons[0].returns_value, "ok");
    }

    #[test]
    fn save_discard_cancel_three_buttons() {
        let opts = save_discard_cancel("Save?", "Unsaved changes");
        assert_eq!(opts.buttons.len(), 3);
        assert_eq!(opts.buttons[0].returns_value, "save");
        assert_eq!(opts.buttons[1].returns_value, "discard");
        assert_eq!(opts.buttons[2].returns_value, "cancel");
    }

    #[test]
    fn is_ok_result_checks() {
        assert!(is_ok_result(&DialogResult::selected("ok")));
        assert!(!is_ok_result(&DialogResult::selected("cancel")));
        assert!(!is_ok_result(&DialogResult::cancelled()));
    }

    #[test]
    fn is_cancel_result_checks() {
        assert!(is_cancel_result(&DialogResult::cancelled()));
        assert!(is_cancel_result(&DialogResult::selected("cancel")));
        assert!(!is_cancel_result(&DialogResult::selected("ok")));
    }

    #[test]
    fn cancelled_dialog_count_works() {
        let mut h = DialogHistory::new();
        h.record("a", DialogKind::Info, DialogResult::cancelled());
        h.record("b", DialogKind::Info, DialogResult::selected("ok"));
        h.record("c", DialogKind::Error, DialogResult::cancelled());
        assert_eq!(cancelled_dialog_count(&h), 2);
    }

    #[test]
    fn most_recent_dialog_returns_last() {
        let mut h = DialogHistory::new();
        h.record("first", DialogKind::Info, DialogResult::cancelled());
        h.record("second", DialogKind::Error, DialogResult::selected("ok"));
        let recent = most_recent_dialog(&h).unwrap();
        assert_eq!(recent.dialog_id, "second");
    }

    #[test]
    fn history_filter_by_kind_filters() {
        let mut h = DialogHistory::new();
        h.record("a", DialogKind::Info, DialogResult::cancelled());
        h.record("b", DialogKind::Error, DialogResult::cancelled());
        h.record("c", DialogKind::Info, DialogResult::selected("ok"));
        let infos = history_filter_by_kind(&h, DialogKind::Info);
        assert_eq!(infos.len(), 2);
    }

    // -----------------------------------------------------------------------
    // DialogButtonLayout tests
    // -----------------------------------------------------------------------

    #[test]
    fn button_layout_total_width_empty() {
        let layout = DialogButtonLayout::new(ButtonAlignment::Center);
        assert_eq!(layout.total_width(), 0);
    }

    #[test]
    fn button_layout_total_width_with_buttons() {
        let mut layout = DialogButtonLayout::new(ButtonAlignment::Right).with_spacing(3);
        layout.add_button(DialogButton::ok());       // label "OK" → 2 + 4 + 2 emphasis = 8
        layout.add_button(DialogButton::cancel());   // label "Cancel" → 6 + 4 = 10
        // 8 + 10 + 3 (one gap) = 21
        assert_eq!(layout.total_width(), 21);
    }

    #[test]
    fn button_layout_left_offset_left() {
        let mut layout = DialogButtonLayout::new(ButtonAlignment::Left);
        layout.add_button(DialogButton::ok());
        assert_eq!(layout.left_offset(80), 0);
    }

    #[test]
    fn button_layout_left_offset_center() {
        let mut layout = DialogButtonLayout::new(ButtonAlignment::Center).with_emphasis(false);
        layout.add_button(DialogButton::new("A", "a")); // 1+4=5
        // total_width = 5, container=20, offset = (20-5)/2 = 7
        assert_eq!(layout.left_offset(20), 7);
    }

    #[test]
    fn button_layout_primary_index() {
        let mut layout = DialogButtonLayout::new(ButtonAlignment::Right);
        layout.add_button(DialogButton::cancel());
        layout.add_button(DialogButton::ok());
        assert_eq!(layout.primary_index(), Some(1));
    }

    #[test]
    fn button_layout_display() {
        let layout = DialogButtonLayout::new(ButtonAlignment::Left);
        let s = format!("{layout}");
        assert!(s.contains("left"));
        assert!(s.contains("0 buttons"));
    }

    // -----------------------------------------------------------------------
    // DialogFormFields tests
    // -----------------------------------------------------------------------

    #[test]
    fn form_fields_validate_required_empty() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::text("name", "Name").required());
        let errors = form.validate();
        assert!(errors.contains_key("name"));
    }

    #[test]
    fn form_fields_validate_passes_when_filled() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::text("name", "Name").required());
        form.set_value("name", "Alice");
        assert!(form.is_valid());
    }

    #[test]
    fn form_fields_dropdown_invalid_option() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::dropdown(
            "color",
            "Colour",
            vec!["red".into(), "blue".into()],
        ));
        form.set_value("color", "green");
        let errors = form.validate();
        assert!(errors.contains_key("color"));
    }

    #[test]
    fn form_fields_focus_wraps() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::text("a", "A"));
        form.add_field(FormField::text("b", "B"));
        assert_eq!(form.focus_index(), 0);
        form.focus_next();
        assert_eq!(form.focus_index(), 1);
        form.focus_next();
        assert_eq!(form.focus_index(), 0); // wraps
    }

    #[test]
    fn form_fields_focus_prev_wraps() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::text("a", "A"));
        form.add_field(FormField::text("b", "B"));
        form.focus_prev(); // wraps from 0 → 1
        assert_eq!(form.focus_index(), 1);
    }

    #[test]
    fn form_fields_checkbox_toggle() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::checkbox("agree", "I agree"));
        let field = form.field_by_name_mut("agree").unwrap();
        assert!(!field.is_checked());
        field.set_checked(true);
        assert!(field.is_checked());
    }

    #[test]
    fn form_fields_values_map() {
        let mut form = DialogFormFields::new();
        form.add_field(FormField::text("x", "X"));
        form.set_value("x", "hello");
        let vals = form.values();
        assert_eq!(vals.get("x").unwrap(), "hello");
    }

    // -----------------------------------------------------------------------
    // DialogProgressIndicator tests
    // -----------------------------------------------------------------------

    #[test]
    fn progress_determinate_lifecycle() {
        let mut p = DialogProgressIndicator::determinate("Loading");
        assert_eq!(p.percentage(), 0);
        assert!(!p.is_complete());
        p.set_percentage(50);
        assert_eq!(p.percentage(), 50);
        p.finish();
        assert!(p.is_complete());
        assert_eq!(p.percentage(), 100);
    }

    #[test]
    fn progress_clamps_at_100() {
        let mut p = DialogProgressIndicator::determinate("test");
        p.set_percentage(200);
        assert_eq!(p.percentage(), 100);
        assert!(p.is_complete());
    }

    #[test]
    fn progress_indeterminate_render() {
        let p = DialogProgressIndicator::indeterminate("Searching");
        let bar = p.render_bar(10);
        assert!(bar.contains("~~~~~~~~~~"));
        assert!(bar.contains("..."));
    }

    #[test]
    fn progress_determinate_render_bar() {
        let mut p = DialogProgressIndicator::determinate("test");
        p.set_percentage(50);
        let bar = p.render_bar(10);
        assert!(bar.contains("50%"));
    }

    // -----------------------------------------------------------------------
    // DialogKeyboardShortcuts tests
    // -----------------------------------------------------------------------

    #[test]
    fn shortcuts_defaults_enter_confirm() {
        let shortcuts = DialogKeyboardShortcuts::with_defaults();
        assert_eq!(
            shortcuts.lookup(&KeyEvent::Enter),
            Some(&DialogAction::Confirm),
        );
        assert_eq!(
            shortcuts.lookup(&KeyEvent::Escape),
            Some(&DialogAction::Cancel),
        );
    }

    #[test]
    fn shortcuts_bind_and_unbind() {
        let mut shortcuts = DialogKeyboardShortcuts::new();
        shortcuts.bind(KeyEvent::F(1), DialogAction::Custom("help".into()));
        assert_eq!(
            shortcuts.lookup(&KeyEvent::F(1)),
            Some(&DialogAction::Custom("help".into())),
        );
        shortcuts.unbind(&KeyEvent::F(1));
        assert!(shortcuts.lookup(&KeyEvent::F(1)).is_none());
    }

    #[test]
    fn shortcuts_rebind_replaces() {
        let mut shortcuts = DialogKeyboardShortcuts::new();
        shortcuts.bind(KeyEvent::Enter, DialogAction::Confirm);
        shortcuts.bind(KeyEvent::Enter, DialogAction::Cancel);
        assert_eq!(
            shortcuts.lookup(&KeyEvent::Enter),
            Some(&DialogAction::Cancel),
        );
        assert_eq!(shortcuts.binding_count(), 1);
    }

    #[test]
    fn shortcuts_keys_for_action() {
        let mut shortcuts = DialogKeyboardShortcuts::new();
        shortcuts.bind(KeyEvent::Enter, DialogAction::Confirm);
        shortcuts.bind(KeyEvent::Char('y'), DialogAction::Confirm);
        let keys = shortcuts.keys_for_action(&DialogAction::Confirm);
        assert_eq!(keys.len(), 2);
    }

    // -- DialogLayoutCalculator -----------------------------------------------

    #[test]
    fn layout_calc_center() {
        let calc = DialogLayoutCalculator::new(100, 50);
        let (x, y) = calc.center(40, 20);
        assert_eq!(x, 30);
        assert_eq!(y, 15);
    }

    #[test]
    fn layout_calc_clamp_size() {
        let calc = DialogLayoutCalculator::new(80, 40).with_margin(5);
        let (w, h) = calc.clamp_size(200, 100);
        assert_eq!(w, 70);
        assert_eq!(h, 30);
    }

    #[test]
    fn layout_calc_compute_rect() {
        let calc = DialogLayoutCalculator::new(100, 50).with_margin(0);
        let (x, y, w, h) = calc.compute_rect(40, 20);
        assert_eq!((x, y, w, h), (30, 15, 40, 20));
    }

    #[test]
    fn layout_calc_available() {
        let calc = DialogLayoutCalculator::new(100, 50).with_margin(10);
        assert_eq!(calc.available_width(), 80);
        assert_eq!(calc.available_height(), 30);
    }

    // -- DialogAnimationState -------------------------------------------------

    #[test]
    fn animation_open_close_lifecycle() {
        let mut anim = DialogAnimationState::new(100);
        assert!(!anim.is_visible());
        anim.start_opening();
        assert!(anim.is_animating());
        assert!(!anim.tick(50));
        assert!(anim.progress() > 0.0 && anim.progress() < 1.0);
        assert!(anim.tick(60));
        assert_eq!(anim.phase(), DialogAnimationPhase::Open);
    }

    #[test]
    fn animation_closing() {
        let mut anim = DialogAnimationState::new(100);
        anim.start_opening();
        anim.tick(200);
        anim.start_closing();
        assert!(anim.is_animating());
        anim.tick(100);
        assert_eq!(anim.phase(), DialogAnimationPhase::Closed);
        assert!(!anim.is_visible());
    }

    #[test]
    fn animation_zero_duration() {
        let mut anim = DialogAnimationState::new(0);
        anim.start_opening();
        let completed = anim.tick(0);
        assert!(completed);
        assert_eq!(anim.phase(), DialogAnimationPhase::Open);
    }

    // -- DialogFocusTrap ------------------------------------------------------

    #[test]
    fn focus_trap_cycle() {
        let mut trap = DialogFocusTrap::new();
        trap.add_element("ok");
        trap.add_element("cancel");
        assert_eq!(trap.focus_first(), Some("ok"));
        assert_eq!(trap.cycle_next(), Some("cancel"));
        assert_eq!(trap.cycle_next(), Some("ok")); // wraps
    }

    #[test]
    fn focus_trap_prev() {
        let mut trap = DialogFocusTrap::new();
        trap.add_element("a");
        trap.add_element("b");
        trap.focus_first();
        assert_eq!(trap.cycle_prev(), Some("b")); // wraps back
    }

    #[test]
    fn focus_trap_restore() {
        let mut trap = DialogFocusTrap::new();
        trap.set_previous_focus("editor");
        assert_eq!(trap.restore_target(), Some("editor"));
    }

    #[test]
    fn focus_trap_empty() {
        let mut trap = DialogFocusTrap::new();
        assert_eq!(trap.focus_first(), None);
        assert_eq!(trap.cycle_next(), None);
        assert!(trap.is_empty());
    }
}
