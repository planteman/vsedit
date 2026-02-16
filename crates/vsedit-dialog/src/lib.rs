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
}
