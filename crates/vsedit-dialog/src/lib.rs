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


/// Configuration manager for dialog functionality.
pub struct DialogConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl DialogConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &DialogConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for dialog operations.
pub struct DialogRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl DialogRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for dialog.
pub struct DialogValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl DialogValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &DialogValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 7
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer7 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer7 {
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
pub fn xb_fnv1a_7(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_7<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_7<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_7(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_7(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 28
// ---------------------------------------------------------------------------

/// Generic object pool `Xc28Pool<T>`.
pub struct Xc28Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc28Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc28PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc28Pool<T> {
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
    pub fn stats(&self) -> Xc28PoolStats {
        Xc28PoolStats {
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

impl<T> Default for Xc28Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc28Scheduler`.
pub struct Xc28Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc28Scheduler {
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

impl Default for Xc28Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_28 hash for the given byte slice.
pub fn xc_28_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_28 convention.
pub fn xc_28_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe17 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe17Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe17PipelineError {
    pub stage: Xe17Stage,
    pub message: String,
}

impl std::fmt::Display for Xe17PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe17Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe17Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError>>>,
    stage_names: Vec<Xe17Stage>,
}

impl Xe17Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe17Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe17Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe17Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe17Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
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

    pub fn compose(mut self, other: Xe17Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe17CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe17CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe17Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe17CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe17CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe17Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe17CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_17_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe17CacheEntry {
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

    fn xe_17_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe17CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_17_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
    Ok(data)
}

pub fn xe_17_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_17_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_17_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_17_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe17PipelineError> {
    Err(Xe17PipelineError {
        stage: Xe17Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #86
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf86Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf86TrieNode {
    children: std::collections::HashMap<char, Xf86TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf86Trie {
    root: Xf86TrieNode,
    count: usize,
}

impl Xf86Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf86TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf86TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf86TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf86BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf86BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 27).
pub struct Xh27SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh27SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 69 as u64,
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

/// A compact bit set supporting boolean operations (variant 27).
pub struct Xh27BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh27BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 27).
pub struct Xi27Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi27Deque<T> {
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
pub struct Xi27Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi27Interval {
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

/// A simple interval tree (variant 27).
pub struct Xi27IntervalTree {
    xi_intervals: Vec<Xi27Interval>,
}

impl Xi27IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi27Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi27Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi27Interval) -> Vec<&Xi27Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi27Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi27Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi27Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi27Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi27Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi27Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 26) ---

/// Disjoint set / union-find for crate 26.
pub struct Xj26UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj26UnionFind {
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

const XJ26_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 26.
pub struct Xj26BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj26BTreeNode<K, V>>>,
    len: usize,
}

struct Xj26BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj26BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj26BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ26_BTREE_ORDER - 1
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
        let mid = XJ26_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj26BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj26BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj26BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj26BTreeNode::xj_new_leaf();
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

    // ----- DialogValidationCollector tests -----

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

    #[test]
    fn dialog_config_new() {
        let cfg = DialogConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn dialog_config_set_get() {
        let mut cfg = DialogConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn dialog_config_remove() {
        let mut cfg = DialogConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn dialog_config_keys_sorted() {
        let mut cfg = DialogConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn dialog_config_bump_version() {
        let mut cfg = DialogConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn dialog_config_clear() {
        let mut cfg = DialogConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn dialog_config_merge() {
        let mut cfg1 = DialogConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = DialogConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn dialog_config_disable() {
        let mut cfg = DialogConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn dialog_rate_tracker_empty() {
        let rt = DialogRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn dialog_rate_tracker_record() {
        let mut rt = DialogRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn dialog_rate_tracker_prune() {
        let mut rt = DialogRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn dialog_validator_valid() {
        let v = DialogValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn dialog_validator_errors() {
        let mut v = DialogValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn dialog_validator_clear() {
        let mut v = DialogValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn dialog_validator_merge() {
        let mut v1 = DialogValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = DialogValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn dialog_rate_tracker_clear() {
        let mut rt = DialogRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_7_push_and_len() {
        let mut rb = super::XbRingBuffer7::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_7_overwrite() {
        let mut rb = super::XbRingBuffer7::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_7_get_out_of_bounds() {
        let rb = super::XbRingBuffer7::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_7_drain_all() {
        let mut rb = super::XbRingBuffer7::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_7_peek_front_back() {
        let mut rb = super::XbRingBuffer7::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_7_clear() {
        let mut rb = super::XbRingBuffer7::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_7_capacity() {
        let rb = super::XbRingBuffer7::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_7_basic() {
        let h = super::xb_fnv1a_7(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_7(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_7_different_inputs() {
        let h1 = super::xb_fnv1a_7(b"abc");
        let h2 = super::xb_fnv1a_7(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_7_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_7(&data);
        let dec = super::xb_rle_decode_7(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_7_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_7(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_7(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_7_values() {
        assert!((super::xb_clamp_7(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_7(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_7(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_7_values() {
        assert!((super::xb_lerp_7(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_7(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_7(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_7_wrap_around_twice() {
        let mut rb = super::XbRingBuffer7::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 28 ----

    #[test]
    fn xc_28_pool_new_empty() {
        let pool: super::Xc28Pool<i32> = super::Xc28Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_28_pool_release_acquire() {
        let mut pool = super::Xc28Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_28_pool_acquire_empty() {
        let mut pool: super::Xc28Pool<i32> = super::Xc28Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_28_pool_full() {
        let mut pool = super::Xc28Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_28_pool_drain() {
        let mut pool = super::Xc28Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_28_pool_stats() {
        let mut pool = super::Xc28Pool::new(8);
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
    fn xc_28_pool_clear() {
        let mut pool = super::Xc28Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_28_pool_shrink() {
        let mut pool = super::Xc28Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_28_pool_default() {
        let pool: super::Xc28Pool<String> = super::Xc28Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_28_pool_extend() {
        let mut pool = super::Xc28Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_28_pool_retain() {
        let mut pool = super::Xc28Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_28_scheduler_round_robin() {
        let mut sched = super::Xc28Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_28_scheduler_empty() {
        let mut sched = super::Xc28Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_28_scheduler_reset() {
        let mut sched = super::Xc28Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_28_scheduler_add_remove() {
        let mut sched = super::Xc28Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_28_scheduler_targets() {
        let sched = super::Xc28Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_28_hash_empty() {
        assert_eq!(super::xc_28_hash(b""), 5381);
    }

    #[test]
    fn xc_28_hash_data() {
        let h = super::xc_28_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_28_hash(b"hello"), h);
    }

    #[test]
    fn xc_28_reverse_str() {
        assert_eq!(super::xc_28_reverse("abc"), "cba");
        assert_eq!(super::xc_28_reverse(""), "");
    }


    #[test]
    fn xe_17_pipeline_empty() {
        let p = super::Xe17Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_17_pipeline_parse_stage() {
        let p = super::Xe17Pipeline::new()
            .add_parse(super::xe_17_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_17_pipeline_transform_double() {
        let p = super::Xe17Pipeline::new()
            .add_transform(super::xe_17_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_17_pipeline_validate_reverse() {
        let p = super::Xe17Pipeline::new()
            .add_validate(super::xe_17_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_17_pipeline_emit_filter() {
        let p = super::Xe17Pipeline::new()
            .add_emit(super::xe_17_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_17_pipeline_multi_stage() {
        let p = super::Xe17Pipeline::new()
            .add_parse(super::xe_17_pipeline_identity)
            .add_transform(super::xe_17_pipeline_double)
            .add_validate(super::xe_17_pipeline_reverse)
            .add_emit(super::xe_17_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_17_pipeline_error_propagation() {
        let p = super::Xe17Pipeline::new()
            .add_parse(super::xe_17_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe17Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_17_pipeline_compose() {
        let p1 = super::Xe17Pipeline::new()
            .add_parse(super::xe_17_pipeline_identity);
        let p2 = super::Xe17Pipeline::new()
            .add_transform(super::xe_17_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_17_pipeline_error_display() {
        let e = super::Xe17PipelineError {
            stage: super::Xe17Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_17_cache_put_get() {
        let mut c = super::Xe17Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_17_cache_miss() {
        let mut c: super::Xe17Cache<&str, i32> = super::Xe17Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_17_cache_ttl_expiry() {
        let mut c = super::Xe17Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_17_cache_evict() {
        let mut c = super::Xe17Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_17_cache_capacity() {
        let mut c = super::Xe17Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_17_cache_stats() {
        let mut c = super::Xe17Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_17_cache_clear() {
        let mut c = super::Xe17Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #86 --

    #[test]
    fn xf86_trie_insert_search() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf86_trie_starts_with() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf86_trie_remove() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf86_trie_word_count() {
        let mut t = Xf86Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf86_trie_longest_prefix() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf86_trie_all_words() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf86_trie_autocomplete() {
        let mut t = Xf86Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf86_trie_empty_search() {
        let t = Xf86Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf86_bloom_add_contains() {
        let mut bf = Xf86BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf86_bloom_probably_absent() {
        let bf = Xf86BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf86_bloom_false_positive_rate() {
        let mut bf = Xf86BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf86_bloom_clear() {
        let mut bf = Xf86BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf86_bloom_union() {
        let mut a = Xf86BloomFilter::xf_new(512, 2);
        let mut b = Xf86BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf86_bloom_intersection_estimate() {
        let mut a = Xf86BloomFilter::xf_new(512, 2);
        let mut b = Xf86BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf86_bloom_union_size_mismatch() {
        let a = Xf86BloomFilter::xf_new(256, 2);
        let b = Xf86BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh27_skip_insert_contains() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh27_skip_remove() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh27_skip_len() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh27_skip_range_query() {
        let mut sl = super::Xh27SkipList::xh_new(4);
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
    fn xh27_skip_floor_ceiling() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh27_skip_rank() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh27_skip_empty() {
        let sl = super::Xh27SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh27_skip_duplicates() {
        let mut sl = super::Xh27SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh27_bitset_set_test() {
        let mut bs = super::Xh27BitSet::xh_new(256);
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
    fn xh27_bitset_clear_count() {
        let mut bs = super::Xh27BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh27_bitset_and_or_xor() {
        let mut a = super::Xh27BitSet::xh_new(128);
        let mut b = super::Xh27BitSet::xh_new(128);
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
    fn xh27_bitset_iter_ones() {
        let mut bs = super::Xh27BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh27_bitset_first_last() {
        let mut bs = super::Xh27BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh27_bitset_empty() {
        let bs = super::Xh27BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi27_deque_push_pop_back() {
        let mut dq = super::Xi27Deque::xi_new(4);
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
    fn xi27_deque_push_pop_front() {
        let mut dq = super::Xi27Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi27_deque_mixed_ops() {
        let mut dq = super::Xi27Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi27_deque_get_and_split() {
        let mut dq = super::Xi27Deque::xi_new(8);
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
    fn xi27_deque_rotate_left() {
        let mut dq = super::Xi27Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi27_deque_rotate_right() {
        let mut dq = super::Xi27Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi27_deque_grow() {
        let mut dq = super::Xi27Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi27_deque_empty() {
        let dq = super::Xi27Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi27_interval_tree_insert_query() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi27Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi27Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi27_interval_tree_overlap() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi27Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi27Interval::xi_new(12, 20));
        let q = super::Xi27Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi27_interval_tree_remove() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi27Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi27_interval_tree_gaps() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi27Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi27Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi27Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi27Interval::xi_new(8, 10));
    }

    #[test]
    fn xi27_interval_tree_merge() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi27Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi27Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi27Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi27Interval::xi_new(10, 15));
    }

    #[test]
    fn xi27_interval_tree_all() {
        let mut tree = super::Xi27IntervalTree::xi_new();
        tree.xi_insert(super::Xi27Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi27Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi27_interval_tree_empty() {
        let tree = super::Xi27IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi27_interval_tree_contains_point() {
        let iv = super::Xi27Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 26) ---

    #[test]
    fn xj_26_uf_make_and_find() {
        let mut uf = super::Xj26UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_26_uf_union_connected() {
        let mut uf = super::Xj26UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_26_uf_component_count() {
        let mut uf = super::Xj26UnionFind::xj_new();
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
    fn xj_26_uf_component_size() {
        let mut uf = super::Xj26UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_26_uf_largest_component() {
        let mut uf = super::Xj26UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_26_uf_many_elements() {
        let mut uf = super::Xj26UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_26_uf_separate_components() {
        let mut uf = super::Xj26UnionFind::xj_new();
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
    fn xj_26_uf_path_compression() {
        let mut uf = super::Xj26UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_26_bt_insert_get() {
        let mut bt = super::Xj26BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_26_bt_contains_len() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_26_bt_replace() {
        let mut bt = super::Xj26BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_26_bt_remove() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_26_bt_keys_values() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_26_bt_range() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_26_bt_min_max() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_26_bt_many_inserts() {
        let mut bt = super::Xj26BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
