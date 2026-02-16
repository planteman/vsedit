//! Editor options and computed config

use std::fmt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WordWrap {
    Off,
    On,
    WordWrapColumn,
    Bounded,
    /// Used for diff settings to mean "inherit from editor".
    Inherit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineNumbersType {
    Off,
    On,
    Relative,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorStyle {
    Line,
    Block,
    Underline,
    LineThin,
    BlockOutline,
    UnderlineThin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorBlinking {
    Blink,
    Smooth,
    Phase,
    Expand,
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderWhitespace {
    None,
    Boundary,
    Selection,
    Trailing,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoClosing {
    Always,
    LanguageDefined,
    BeforeWhitespace,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoSurround {
    LanguageDefined,
    Quotes,
    Brackets,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoIndent {
    None,
    Keep,
    Brackets,
    Advanced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AcceptSuggestionOnEnter {
    On,
    Smart,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnippetSuggestions {
    Top,
    Bottom,
    Inline,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MinimapSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffAlgorithm {
    Legacy,
    Advanced,
}

// ---------------------------------------------------------------------------
// EditorOptions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub detect_indentation: bool,
    pub trim_auto_whitespace: bool,
    pub word_wrap: WordWrap,
    pub word_wrap_column: u32,
    pub line_numbers: LineNumbersType,
    pub minimap_enabled: bool,
    pub minimap_side: MinimapSide,
    pub cursor_style: CursorStyle,
    pub cursor_blinking: CursorBlinking,
    pub cursor_width: u32,
    pub scroll_beyond_last_line: bool,
    pub smooth_scrolling: bool,
    pub font_size: u32,
    pub line_height: u32,
    pub render_whitespace: RenderWhitespace,
    pub render_control_characters: bool,
    pub rulers: Vec<u32>,
    pub auto_closing_brackets: AutoClosing,
    pub auto_surround: AutoSurround,
    pub auto_indent: AutoIndent,
    pub format_on_paste: bool,
    pub format_on_type: bool,
    pub format_on_save: bool,
    pub suggest_on_trigger_characters: bool,
    pub accept_suggestion_on_enter: AcceptSuggestionOnEnter,
    pub snippet_suggestions: SnippetSuggestions,
    pub quick_suggestions: bool,
    pub bracket_pair_colorization: bool,
    pub guides_indentation: bool,
    pub guides_bracket_pairs: bool,
    pub folding_enabled: bool,
    pub links: bool,
    pub sticky_scroll_enabled: bool,
    pub sticky_scroll_max_line_count: u32,
    pub diff_word_wrap: WordWrap,
    pub diff_algorithm: DiffAlgorithm,
}

impl Default for EditorOptions {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            detect_indentation: true,
            trim_auto_whitespace: true,
            word_wrap: WordWrap::Off,
            word_wrap_column: 80,
            line_numbers: LineNumbersType::On,
            minimap_enabled: true,
            minimap_side: MinimapSide::Right,
            cursor_style: CursorStyle::Line,
            cursor_blinking: CursorBlinking::Blink,
            cursor_width: 0,
            scroll_beyond_last_line: true,
            smooth_scrolling: true,
            font_size: 14,
            line_height: 0,
            render_whitespace: RenderWhitespace::Selection,
            render_control_characters: true,
            rulers: Vec::new(),
            auto_closing_brackets: AutoClosing::LanguageDefined,
            auto_surround: AutoSurround::LanguageDefined,
            auto_indent: AutoIndent::Advanced,
            format_on_paste: false,
            format_on_type: false,
            format_on_save: false,
            suggest_on_trigger_characters: true,
            accept_suggestion_on_enter: AcceptSuggestionOnEnter::On,
            snippet_suggestions: SnippetSuggestions::Inline,
            quick_suggestions: true,
            bracket_pair_colorization: true,
            guides_indentation: true,
            guides_bracket_pairs: true,
            folding_enabled: true,
            links: true,
            sticky_scroll_enabled: true,
            sticky_scroll_max_line_count: 5,
            diff_word_wrap: WordWrap::Inherit,
            diff_algorithm: DiffAlgorithm::Advanced,
        }
    }
}

impl EditorOptions {
    /// Overlay JSON settings onto the current options.
    /// Only fields present in the JSON object are updated.
    pub fn merge_from_json(&mut self, json: &Value) {
        let obj = match json.as_object() {
            Some(o) => o,
            None => return,
        };

        macro_rules! merge_field {
            ($field:ident, $key:expr, u32) => {
                if let Some(v) = obj.get($key).and_then(Value::as_u64) {
                    self.$field = v as u32;
                }
            };
            ($field:ident, $key:expr, bool) => {
                if let Some(v) = obj.get($key).and_then(Value::as_bool) {
                    self.$field = v;
                }
            };
            ($field:ident, $key:expr, enum) => {
                if let Some(v) = obj.get($key) {
                    if let Ok(parsed) = serde_json::from_value(v.clone()) {
                        self.$field = parsed;
                    }
                }
            };
            ($field:ident, $key:expr, vec_u32) => {
                if let Some(arr) = obj.get($key).and_then(Value::as_array) {
                    self.$field = arr
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u32))
                        .collect();
                }
            };
        }

        merge_field!(tab_size, "tabSize", u32);
        merge_field!(insert_spaces, "insertSpaces", bool);
        merge_field!(detect_indentation, "detectIndentation", bool);
        merge_field!(trim_auto_whitespace, "trimAutoWhitespace", bool);
        merge_field!(word_wrap, "wordWrap", enum);
        merge_field!(word_wrap_column, "wordWrapColumn", u32);
        merge_field!(line_numbers, "lineNumbers", enum);
        merge_field!(minimap_enabled, "minimapEnabled", bool);
        merge_field!(minimap_side, "minimapSide", enum);
        merge_field!(cursor_style, "cursorStyle", enum);
        merge_field!(cursor_blinking, "cursorBlinking", enum);
        merge_field!(cursor_width, "cursorWidth", u32);
        merge_field!(scroll_beyond_last_line, "scrollBeyondLastLine", bool);
        merge_field!(smooth_scrolling, "smoothScrolling", bool);
        merge_field!(font_size, "fontSize", u32);
        merge_field!(line_height, "lineHeight", u32);
        merge_field!(render_whitespace, "renderWhitespace", enum);
        merge_field!(render_control_characters, "renderControlCharacters", bool);
        merge_field!(rulers, "rulers", vec_u32);
        merge_field!(auto_closing_brackets, "autoClosingBrackets", enum);
        merge_field!(auto_surround, "autoSurround", enum);
        merge_field!(auto_indent, "autoIndent", enum);
        merge_field!(format_on_paste, "formatOnPaste", bool);
        merge_field!(format_on_type, "formatOnType", bool);
        merge_field!(format_on_save, "formatOnSave", bool);
        merge_field!(suggest_on_trigger_characters, "suggestOnTriggerCharacters", bool);
        merge_field!(accept_suggestion_on_enter, "acceptSuggestionOnEnter", enum);
        merge_field!(snippet_suggestions, "snippetSuggestions", enum);
        merge_field!(quick_suggestions, "quickSuggestions", bool);
        merge_field!(bracket_pair_colorization, "bracketPairColorization", bool);
        merge_field!(guides_indentation, "guidesIndentation", bool);
        merge_field!(guides_bracket_pairs, "guidesBracketPairs", bool);
        merge_field!(folding_enabled, "foldingEnabled", bool);
        merge_field!(links, "links", bool);
        merge_field!(sticky_scroll_enabled, "stickyScrollEnabled", bool);
        merge_field!(sticky_scroll_max_line_count, "stickyScrollMaxLineCount", u32);
        merge_field!(diff_word_wrap, "diffWordWrap", enum);
        merge_field!(diff_algorithm, "diffAlgorithm", enum);
    }

    /// Serialize the current options to a JSON value.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("EditorOptions serialization should never fail")
    }

    /// Returns true if rulers is empty.
    pub fn is_rulers_empty(&self) -> bool {
        self.rulers.is_empty()
    }

    /// Get the first ruler, if any.
    pub fn first_ruler(&self) -> Option<&u32> {
        self.rulers.first()
    }

    /// Get the last ruler, if any.
    pub fn last_ruler(&self) -> Option<&u32> {
        self.rulers.last()
    }

    /// Retain only rulers matching the predicate.
    pub fn retain_rulers(&mut self, f: impl Fn(&u32) -> bool) {
        self.rulers.retain(|item| f(item));
    }

    /// Toggle the `insert_spaces` flag.
    pub fn toggle_insert_spaces(&mut self) {
        self.insert_spaces = !self.insert_spaces;
    }

    /// Toggle the `detect_indentation` flag.
    pub fn toggle_detect_indentation(&mut self) {
        self.detect_indentation = !self.detect_indentation;
    }

    /// Toggle the `trim_auto_whitespace` flag.
    pub fn toggle_trim_auto_whitespace(&mut self) {
        self.trim_auto_whitespace = !self.trim_auto_whitespace;
    }

    /// Toggle the `minimap_enabled` flag.
    pub fn toggle_minimap_enabled(&mut self) {
        self.minimap_enabled = !self.minimap_enabled;
    }

    /// Toggle the `scroll_beyond_last_line` flag.
    pub fn toggle_scroll_beyond_last_line(&mut self) {
        self.scroll_beyond_last_line = !self.scroll_beyond_last_line;
    }

    /// Toggle the `smooth_scrolling` flag.
    pub fn toggle_smooth_scrolling(&mut self) {
        self.smooth_scrolling = !self.smooth_scrolling;
    }

    /// Toggle the `render_control_characters` flag.
    pub fn toggle_render_control_characters(&mut self) {
        self.render_control_characters = !self.render_control_characters;
    }

    /// Toggle the `format_on_paste` flag.
    pub fn toggle_format_on_paste(&mut self) {
        self.format_on_paste = !self.format_on_paste;
    }

    /// Toggle the `format_on_type` flag.
    pub fn toggle_format_on_type(&mut self) {
        self.format_on_type = !self.format_on_type;
    }

    /// Toggle the `format_on_save` flag.
    pub fn toggle_format_on_save(&mut self) {
        self.format_on_save = !self.format_on_save;
    }

    /// Toggle the `suggest_on_trigger_characters` flag.
    pub fn toggle_suggest_on_trigger_characters(&mut self) {
        self.suggest_on_trigger_characters = !self.suggest_on_trigger_characters;
    }

    /// Toggle the `quick_suggestions` flag.
    pub fn toggle_quick_suggestions(&mut self) {
        self.quick_suggestions = !self.quick_suggestions;
    }

    /// Toggle the `bracket_pair_colorization` flag.
    pub fn toggle_bracket_pair_colorization(&mut self) {
        self.bracket_pair_colorization = !self.bracket_pair_colorization;
    }

    /// Toggle the `guides_indentation` flag.
    pub fn toggle_guides_indentation(&mut self) {
        self.guides_indentation = !self.guides_indentation;
    }

    /// Toggle the `guides_bracket_pairs` flag.
    pub fn toggle_guides_bracket_pairs(&mut self) {
        self.guides_bracket_pairs = !self.guides_bracket_pairs;
    }

    /// Toggle the `folding_enabled` flag.
    pub fn toggle_folding_enabled(&mut self) {
        self.folding_enabled = !self.folding_enabled;
    }

    /// Toggle the `links` flag.
    pub fn toggle_links(&mut self) {
        self.links = !self.links;
    }

    /// Toggle the `sticky_scroll_enabled` flag.
    pub fn toggle_sticky_scroll_enabled(&mut self) {
        self.sticky_scroll_enabled = !self.sticky_scroll_enabled;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for editor-config operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorConfigStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EditorConfigStats {
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
    pub fn merge(&mut self, other: &EditorConfigStats) {
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

impl Default for EditorConfigStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorConfigStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorConfigStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for editor-config.
#[derive(Debug, Clone)]
pub struct EditorConfigValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EditorConfigValidator {
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

impl Default for EditorConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Per-file-type config overrides
// ---------------------------------------------------------------------------

/// A set of editor config overrides keyed by a file-type glob pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorConfigOverride {
    /// Glob pattern, e.g. "*.rs", "*.md", "Makefile".
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_spaces: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_wrap: Option<WordWrap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rulers: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_trailing_whitespace: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_final_newline: Option<bool>,
}

impl EditorConfigOverride {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            tab_size: None,
            insert_spaces: None,
            word_wrap: None,
            rulers: None,
            trim_trailing_whitespace: None,
            insert_final_newline: None,
        }
    }

    pub fn with_tab_size(mut self, size: u32) -> Self {
        self.tab_size = Some(size);
        self
    }

    pub fn with_insert_spaces(mut self, val: bool) -> Self {
        self.insert_spaces = Some(val);
        self
    }

    pub fn with_word_wrap(mut self, wrap: WordWrap) -> Self {
        self.word_wrap = Some(wrap);
        self
    }

    pub fn with_rulers(mut self, rulers: Vec<u32>) -> Self {
        self.rulers = Some(rulers);
        self
    }

    pub fn with_trim_trailing_whitespace(mut self, val: bool) -> Self {
        self.trim_trailing_whitespace = Some(val);
        self
    }

    pub fn with_insert_final_newline(mut self, val: bool) -> Self {
        self.insert_final_newline = Some(val);
        self
    }

    /// Check if a filename matches this override's pattern.
    pub fn matches(&self, filename: &str) -> bool {
        if self.pattern.starts_with('*') {
            let suffix = &self.pattern[1..];
            filename.ends_with(suffix)
        } else {
            filename == self.pattern
        }
    }
}

/// Registry of per-file-type overrides.
#[derive(Debug, Clone, Default)]
pub struct OverrideRegistry {
    overrides: Vec<EditorConfigOverride>,
}

impl OverrideRegistry {
    pub fn new() -> Self {
        Self { overrides: Vec::new() }
    }

    pub fn add(&mut self, ov: EditorConfigOverride) {
        self.overrides.push(ov);
    }

    /// Find the first override whose pattern matches the given filename.
    pub fn find(&self, filename: &str) -> Option<&EditorConfigOverride> {
        self.overrides.iter().find(|o| o.matches(filename))
    }

    /// Find all overrides matching the filename, in registration order.
    pub fn find_all(&self, filename: &str) -> Vec<&EditorConfigOverride> {
        self.overrides.iter().filter(|o| o.matches(filename)).collect()
    }

    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CursorStyle helpers
// ---------------------------------------------------------------------------

impl CursorStyle {
    /// Returns all cursor style variants.
    pub fn all() -> &'static [CursorStyle] {
        &[
            CursorStyle::Line,
            CursorStyle::Block,
            CursorStyle::Underline,
            CursorStyle::LineThin,
            CursorStyle::BlockOutline,
            CursorStyle::UnderlineThin,
        ]
    }

    /// Parse from a string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "line" => Some(Self::Line),
            "block" => Some(Self::Block),
            "underline" => Some(Self::Underline),
            "line-thin" | "linethin" => Some(Self::LineThin),
            "block-outline" | "blockoutline" => Some(Self::BlockOutline),
            "underline-thin" | "underlinethin" => Some(Self::UnderlineThin),
            _ => None,
        }
    }

    /// Returns the cursor character representation.
    pub fn cursor_char(&self) -> char {
        match self {
            CursorStyle::Line | CursorStyle::LineThin => '│',
            CursorStyle::Block | CursorStyle::BlockOutline => '█',
            CursorStyle::Underline | CursorStyle::UnderlineThin => '_',
        }
    }
}

impl fmt::Display for CursorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CursorStyle::Line => write!(f, "line"),
            CursorStyle::Block => write!(f, "block"),
            CursorStyle::Underline => write!(f, "underline"),
            CursorStyle::LineThin => write!(f, "line-thin"),
            CursorStyle::BlockOutline => write!(f, "block-outline"),
            CursorStyle::UnderlineThin => write!(f, "underline-thin"),
        }
    }
}

// ---------------------------------------------------------------------------
// WordWrap helpers
// ---------------------------------------------------------------------------

impl WordWrap {
    /// Returns all word wrap variants.
    pub fn all() -> &'static [WordWrap] {
        &[WordWrap::Off, WordWrap::On, WordWrap::WordWrapColumn, WordWrap::Bounded, WordWrap::Inherit]
    }

    /// Returns true if wrapping is enabled in some form.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, WordWrap::Off | WordWrap::Inherit)
    }
}

impl fmt::Display for WordWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WordWrap::Off => write!(f, "off"),
            WordWrap::On => write!(f, "on"),
            WordWrap::WordWrapColumn => write!(f, "wordWrapColumn"),
            WordWrap::Bounded => write!(f, "bounded"),
            WordWrap::Inherit => write!(f, "inherit"),
        }
    }
}

// ---------------------------------------------------------------------------
// LineNumbersType helpers
// ---------------------------------------------------------------------------

impl LineNumbersType {
    /// Returns all line numbers type variants.
    pub fn all() -> &'static [LineNumbersType] {
        &[
            LineNumbersType::Off,
            LineNumbersType::On,
            LineNumbersType::Relative,
            LineNumbersType::Interval,
        ]
    }

    /// Returns true if line numbers are shown.
    pub fn is_visible(&self) -> bool {
        !matches!(self, LineNumbersType::Off)
    }
}

impl fmt::Display for LineNumbersType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineNumbersType::Off => write!(f, "off"),
            LineNumbersType::On => write!(f, "on"),
            LineNumbersType::Relative => write!(f, "relative"),
            LineNumbersType::Interval => write!(f, "interval"),
        }
    }
}

// ---------------------------------------------------------------------------
// EditorOptions diff
// ---------------------------------------------------------------------------

/// Describes a single changed field between two EditorOptions.
#[derive(Debug, Clone)]
pub struct EditorOptionChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

impl fmt::Display for EditorOptionChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} -> {}", self.field, self.old_value, self.new_value)
    }
}

/// Compare two EditorOptions and return a list of field differences.
pub fn diff_editor_options(a: &EditorOptions, b: &EditorOptions) -> Vec<EditorOptionChange> {
    let mut changes = Vec::new();
    if a.font_size != b.font_size {
        changes.push(EditorOptionChange {
            field: "font_size".to_string(),
            old_value: format!("{}", a.font_size),
            new_value: format!("{}", b.font_size),
        });
    }
    if a.tab_size != b.tab_size {
        changes.push(EditorOptionChange {
            field: "tab_size".to_string(),
            old_value: format!("{}", a.tab_size),
            new_value: format!("{}", b.tab_size),
        });
    }
    if a.insert_spaces != b.insert_spaces {
        changes.push(EditorOptionChange {
            field: "insert_spaces".to_string(),
            old_value: format!("{}", a.insert_spaces),
            new_value: format!("{}", b.insert_spaces),
        });
    }
    if a.word_wrap != b.word_wrap {
        changes.push(EditorOptionChange {
            field: "word_wrap".to_string(),
            old_value: format!("{:?}", a.word_wrap),
            new_value: format!("{:?}", b.word_wrap),
        });
    }
    if a.cursor_style != b.cursor_style {
        changes.push(EditorOptionChange {
            field: "cursor_style".to_string(),
            old_value: format!("{:?}", a.cursor_style),
            new_value: format!("{:?}", b.cursor_style),
        });
    }
    changes
}


// ---------------------------------------------------------------------------
// EditorConfigProfile
// ---------------------------------------------------------------------------

/// A named profile bundling a set of editor options.
#[derive(Debug, Clone)]
pub struct EditorConfigProfile {
    pub name: String,
    pub options: EditorOptions,
    pub is_default: bool,
}

impl EditorConfigProfile {
    pub fn new(name: impl Into<String>, options: EditorOptions) -> Self {
        Self { name: name.into(), options, is_default: false }
    }

    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }
}

impl fmt::Display for EditorConfigProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = if self.is_default { " (default)" } else { "" };
        write!(f, "Profile '{}'{}", self.name, tag)
    }
}

// ---------------------------------------------------------------------------
// ProfileManager
// ---------------------------------------------------------------------------

/// Manages a collection of editor config profiles with one active profile.
#[derive(Debug)]
pub struct ProfileManager {
    profiles: Vec<EditorConfigProfile>,
    active_name: Option<String>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self { profiles: Vec::new(), active_name: None }
    }

    pub fn add(&mut self, profile: EditorConfigProfile) -> bool {
        if self.profiles.iter().any(|p| p.name == profile.name) {
            return false;
        }
        if profile.is_default || self.active_name.is_none() {
            self.active_name = Some(profile.name.clone());
        }
        self.profiles.push(profile);
        true
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        if self.active_name.as_deref() == Some(name) {
            self.active_name = self.profiles.first().map(|p| p.name.clone());
        }
        self.profiles.len() < before
    }

    pub fn activate(&mut self, name: &str) -> bool {
        if self.profiles.iter().any(|p| p.name == name) {
            self.active_name = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn active(&self) -> Option<&EditorConfigProfile> {
        self.active_name
            .as_deref()
            .and_then(|n| self.profiles.iter().find(|p| p.name == n))
    }

    pub fn list(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.name.as_str()).collect()
    }

    pub fn clone_profile(&mut self, source: &str, new_name: impl Into<String>) -> bool {
        let new_name = new_name.into();
        if self.profiles.iter().any(|p| p.name == new_name) {
            return false;
        }
        if let Some(src) = self.profiles.iter().find(|p| p.name == source) {
            let mut cloned = src.clone();
            cloned.name = new_name;
            cloned.is_default = false;
            self.profiles.push(cloned);
            true
        } else {
            false
        }
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorConfigMerger
// ---------------------------------------------------------------------------

pub struct EditorConfigMerger;

impl EditorConfigMerger {
    pub fn merge(base: &EditorOptions, overrides: &EditorOptions) -> EditorOptions {
        let def = EditorOptions::default();
        let mut result = base.clone();
        if overrides.tab_size != def.tab_size { result.tab_size = overrides.tab_size; }
        if overrides.insert_spaces != def.insert_spaces { result.insert_spaces = overrides.insert_spaces; }
        if overrides.word_wrap != def.word_wrap { result.word_wrap = overrides.word_wrap; }
        if overrides.word_wrap_column != def.word_wrap_column { result.word_wrap_column = overrides.word_wrap_column; }
        if overrides.font_size != def.font_size { result.font_size = overrides.font_size; }
        if overrides.line_height != def.line_height { result.line_height = overrides.line_height; }
        if overrides.cursor_style != def.cursor_style { result.cursor_style = overrides.cursor_style; }
        if overrides.cursor_blinking != def.cursor_blinking { result.cursor_blinking = overrides.cursor_blinking; }
        if overrides.line_numbers != def.line_numbers { result.line_numbers = overrides.line_numbers; }
        if overrides.minimap_enabled != def.minimap_enabled { result.minimap_enabled = overrides.minimap_enabled; }
        if overrides.render_whitespace != def.render_whitespace { result.render_whitespace = overrides.render_whitespace; }
        if !overrides.rulers.is_empty() { result.rulers = overrides.rulers.clone(); }
        result
    }
}

// ---------------------------------------------------------------------------
// ConfigValidator
// ---------------------------------------------------------------------------

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(opts: &EditorOptions) -> Vec<String> {
        let mut errors = Vec::new();
        if opts.tab_size == 0 || opts.tab_size > 16 {
            errors.push(format!("tab_size must be 1..16, got {}", opts.tab_size));
        }
        if opts.font_size == 0 || opts.font_size > 200 {
            errors.push(format!("font_size must be 1..200, got {}", opts.font_size));
        }
        if opts.word_wrap_column == 0 || opts.word_wrap_column > 10000 {
            errors.push(format!("word_wrap_column must be 1..10000, got {}", opts.word_wrap_column));
        }
        if opts.sticky_scroll_max_line_count > 50 {
            errors.push(format!(
                "sticky_scroll_max_line_count must be <=50, got {}",
                opts.sticky_scroll_max_line_count,
            ));
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_values() {
        let opts = EditorOptions::default();
        assert_eq!(opts.tab_size, 4);
        assert!(opts.insert_spaces);
        assert!(opts.detect_indentation);
        assert!(opts.trim_auto_whitespace);
        assert_eq!(opts.word_wrap, WordWrap::Off);
        assert_eq!(opts.word_wrap_column, 80);
        assert_eq!(opts.line_numbers, LineNumbersType::On);
        assert!(opts.minimap_enabled);
        assert_eq!(opts.minimap_side, MinimapSide::Right);
        assert_eq!(opts.cursor_style, CursorStyle::Line);
        assert_eq!(opts.cursor_blinking, CursorBlinking::Blink);
        assert_eq!(opts.cursor_width, 0);
        assert!(opts.scroll_beyond_last_line);
        assert!(opts.smooth_scrolling);
        assert_eq!(opts.font_size, 14);
        assert_eq!(opts.line_height, 0);
        assert_eq!(opts.render_whitespace, RenderWhitespace::Selection);
        assert!(opts.render_control_characters);
        assert!(opts.rulers.is_empty());
        assert_eq!(opts.auto_closing_brackets, AutoClosing::LanguageDefined);
        assert_eq!(opts.auto_surround, AutoSurround::LanguageDefined);
        assert_eq!(opts.auto_indent, AutoIndent::Advanced);
        assert!(!opts.format_on_paste);
        assert!(!opts.format_on_type);
        assert!(!opts.format_on_save);
        assert!(opts.suggest_on_trigger_characters);
        assert_eq!(opts.accept_suggestion_on_enter, AcceptSuggestionOnEnter::On);
        assert_eq!(opts.snippet_suggestions, SnippetSuggestions::Inline);
        assert!(opts.quick_suggestions);
        assert!(opts.bracket_pair_colorization);
        assert!(opts.guides_indentation);
        assert!(opts.guides_bracket_pairs);
        assert!(opts.folding_enabled);
        assert!(opts.links);
        assert!(opts.sticky_scroll_enabled);
        assert_eq!(opts.sticky_scroll_max_line_count, 5);
        assert_eq!(opts.diff_word_wrap, WordWrap::Inherit);
        assert_eq!(opts.diff_algorithm, DiffAlgorithm::Advanced);
    }

    #[test]
    fn test_merge_from_json_partial() {
        let mut opts = EditorOptions::default();
        let patch = json!({
            "tabSize": 2,
            "insertSpaces": false,
            "wordWrap": "on",
            "fontSize": 16,
            "rulers": [80, 120],
            "cursorStyle": "block",
            "renderWhitespace": "all",
            "autoIndent": "full",
            "formatOnSave": true,
            "stickyScrollMaxLineCount": 10,
            "diffAlgorithm": "legacy"
        });
        opts.merge_from_json(&patch);

        assert_eq!(opts.tab_size, 2);
        assert!(!opts.insert_spaces);
        assert_eq!(opts.word_wrap, WordWrap::On);
        assert_eq!(opts.font_size, 16);
        assert_eq!(opts.rulers, vec![80, 120]);
        assert_eq!(opts.cursor_style, CursorStyle::Block);
        assert_eq!(opts.render_whitespace, RenderWhitespace::All);
        assert_eq!(opts.auto_indent, AutoIndent::Full);
        assert!(opts.format_on_save);
        assert_eq!(opts.sticky_scroll_max_line_count, 10);
        assert_eq!(opts.diff_algorithm, DiffAlgorithm::Legacy);

        // Unmentioned fields keep defaults
        assert!(opts.detect_indentation);
        assert_eq!(opts.word_wrap_column, 80);
        assert_eq!(opts.line_numbers, LineNumbersType::On);
        assert!(opts.minimap_enabled);
        assert!(opts.smooth_scrolling);
    }

    #[test]
    fn test_merge_ignores_invalid_values() {
        let mut opts = EditorOptions::default();
        let patch = json!({
            "tabSize": "not_a_number",
            "insertSpaces": 42,
            "wordWrap": "invalid_variant",
            "fontSize": -1
        });
        opts.merge_from_json(&patch);

        // All values should remain at defaults
        assert_eq!(opts.tab_size, 4);
        assert!(opts.insert_spaces);
        assert_eq!(opts.word_wrap, WordWrap::Off);
        assert_eq!(opts.font_size, 14);
    }

    #[test]
    fn test_merge_with_non_object_is_noop() {
        let mut opts = EditorOptions::default();
        opts.merge_from_json(&json!(42));
        assert_eq!(opts, EditorOptions::default());

        opts.merge_from_json(&json!(null));
        assert_eq!(opts, EditorOptions::default());
    }

    #[test]
    fn test_to_json() {
        let opts = EditorOptions::default();
        let v = opts.to_json();
        assert_eq!(v["tabSize"], 4);
        assert_eq!(v["insertSpaces"], true);
        assert_eq!(v["wordWrap"], "off");
        assert_eq!(v["lineNumbers"], "on");
        assert_eq!(v["cursorStyle"], "line");
        assert_eq!(v["renderWhitespace"], "selection");
        assert_eq!(v["diffAlgorithm"], "advanced");
    }

    #[test]
    fn test_round_trip() {
        let original = EditorOptions {
            tab_size: 2,
            insert_spaces: false,
            rulers: vec![80, 100, 120],
            word_wrap: WordWrap::Bounded,
            cursor_style: CursorStyle::BlockOutline,
            cursor_blinking: CursorBlinking::Expand,
            render_whitespace: RenderWhitespace::Trailing,
            auto_closing_brackets: AutoClosing::BeforeWhitespace,
            auto_surround: AutoSurround::Brackets,
            auto_indent: AutoIndent::Full,
            accept_suggestion_on_enter: AcceptSuggestionOnEnter::Smart,
            snippet_suggestions: SnippetSuggestions::Top,
            minimap_side: MinimapSide::Left,
            diff_algorithm: DiffAlgorithm::Legacy,
            diff_word_wrap: WordWrap::On,
            ..EditorOptions::default()
        };

        let serialized = original.to_json();
        let mut restored = EditorOptions::default();
        restored.merge_from_json(&serialized);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_serde_round_trip() {
        let original = EditorOptions::default();
        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: EditorOptions = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn eq_wordwrap_same() {
        assert_eq!(WordWrap::Off, WordWrap::Off);
    }

    #[test]
    fn ne_wordwrap_diff() {
        assert_ne!(WordWrap::Off, WordWrap::On);
    }

    #[test]
    fn eq_linenumberstype_same() {
        assert_eq!(LineNumbersType::Off, LineNumbersType::Off);
    }

    #[test]
    fn ne_linenumberstype_diff() {
        assert_ne!(LineNumbersType::Off, LineNumbersType::On);
    }

    #[test]
    fn eq_cursorstyle_same() {
        assert_eq!(CursorStyle::Line, CursorStyle::Line);
    }

    #[test]
    fn ne_cursorstyle_diff() {
        assert_ne!(CursorStyle::Line, CursorStyle::Block);
    }

    #[test]
    fn eq_cursorblinking_same() {
        assert_eq!(CursorBlinking::Blink, CursorBlinking::Blink);
    }

    #[test]
    fn ne_cursorblinking_diff() {
        assert_ne!(CursorBlinking::Blink, CursorBlinking::Smooth);
    }

    #[test]
    fn eq_renderwhitespace_same() {
        assert_eq!(RenderWhitespace::None, RenderWhitespace::None);
    }

    #[test]
    fn ne_renderwhitespace_diff() {
        assert_ne!(RenderWhitespace::None, RenderWhitespace::Boundary);
    }

    #[test]
    fn eq_autoclosing_same() {
        assert_eq!(AutoClosing::Always, AutoClosing::Always);
    }

    #[test]
    fn ne_autoclosing_diff() {
        assert_ne!(AutoClosing::Always, AutoClosing::LanguageDefined);
    }

    #[test]
    fn eq_autosurround_same() {
        assert_eq!(AutoSurround::LanguageDefined, AutoSurround::LanguageDefined);
    }

    #[test]
    fn ne_autosurround_diff() {
        assert_ne!(AutoSurround::LanguageDefined, AutoSurround::Quotes);
    }

    #[test]
    fn eq_autoindent_same() {
        assert_eq!(AutoIndent::None, AutoIndent::None);
    }

    #[test]
    fn ne_autoindent_diff() {
        assert_ne!(AutoIndent::None, AutoIndent::Keep);
    }

    #[test]
    fn eq_acceptsuggestiononenter_same() {
        assert_eq!(AcceptSuggestionOnEnter::On, AcceptSuggestionOnEnter::On);
    }

    #[test]
    fn ne_acceptsuggestiononenter_diff() {
        assert_ne!(AcceptSuggestionOnEnter::On, AcceptSuggestionOnEnter::Smart);
    }

    #[test]
    fn eq_snippetsuggestions_same() {
        assert_eq!(SnippetSuggestions::Top, SnippetSuggestions::Top);
    }

    #[test]
    fn ne_snippetsuggestions_diff() {
        assert_ne!(SnippetSuggestions::Top, SnippetSuggestions::Bottom);
    }

    #[test]
    fn eq_minimapside_same() {
        assert_eq!(MinimapSide::Left, MinimapSide::Left);
    }

    #[test]
    fn ne_minimapside_diff() {
        assert_ne!(MinimapSide::Left, MinimapSide::Right);
    }

    #[test]
    fn eq_diffalgorithm_same() {
        assert_eq!(DiffAlgorithm::Legacy, DiffAlgorithm::Legacy);
    }

    #[test]
    fn ne_diffalgorithm_diff() {
        assert_ne!(DiffAlgorithm::Legacy, DiffAlgorithm::Advanced);
    }

    #[test]
    fn editor_config_stats_new_defaults() {
        let stats = EditorConfigStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn editor_config_stats_record_success() {
        let mut stats = EditorConfigStats::new();
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
    fn editor_config_stats_record_failure() {
        let mut stats = EditorConfigStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn editor_config_stats_reset() {
        let mut stats = EditorConfigStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn editor_config_stats_merge() {
        let mut a = EditorConfigStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EditorConfigStats::new();
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
    fn editor_config_stats_display() {
        let mut stats = EditorConfigStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn editor_config_stats_default() {
        let stats = EditorConfigStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn editor_config_validator_accepts_valid_name() {
        let v = EditorConfigValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn editor_config_validator_rejects_empty() {
        let v = EditorConfigValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn editor_config_validator_rejects_too_long() {
        let v = EditorConfigValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn editor_config_validator_forbidden_prefix() {
        let v = EditorConfigValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn editor_config_validator_allowed_chars() {
        let v = EditorConfigValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn editor_config_validator_range() {
        let v = EditorConfigValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn editor_config_sanitize_removes_control() {
        let result = EditorConfigValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn editor_config_truncate_short_string() {
        assert_eq!(EditorConfigValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn editor_config_truncate_long_string() {
        let result = EditorConfigValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn editor_config_is_ascii_printable() {
        assert!(EditorConfigValidator::is_ascii_printable("Hello World 123"));
        assert!(!EditorConfigValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- EditorConfigOverride --

    #[test]
    fn override_matches_glob() {
        let ov = EditorConfigOverride::new("*.rs").with_tab_size(4);
        assert!(ov.matches("main.rs"));
        assert!(ov.matches("lib.rs"));
        assert!(!ov.matches("main.py"));
    }

    #[test]
    fn override_matches_exact() {
        let ov = EditorConfigOverride::new("Makefile");
        assert!(ov.matches("Makefile"));
        assert!(!ov.matches("Makefile.bak"));
    }

    #[test]
    fn override_builder_chain() {
        let ov = EditorConfigOverride::new("*.md")
            .with_word_wrap(WordWrap::On)
            .with_rulers(vec![80])
            .with_trim_trailing_whitespace(true)
            .with_insert_final_newline(true);
        assert_eq!(ov.word_wrap, Some(WordWrap::On));
        assert_eq!(ov.rulers, Some(vec![80]));
        assert_eq!(ov.trim_trailing_whitespace, Some(true));
    }

    #[test]
    fn override_registry_find() {
        let mut reg = OverrideRegistry::new();
        reg.add(EditorConfigOverride::new("*.rs").with_tab_size(4));
        reg.add(EditorConfigOverride::new("*.py").with_tab_size(2));
        assert_eq!(reg.find("test.rs").unwrap().tab_size, Some(4));
        assert_eq!(reg.find("test.py").unwrap().tab_size, Some(2));
        assert!(reg.find("test.js").is_none());
    }

    #[test]
    fn override_registry_find_all() {
        let mut reg = OverrideRegistry::new();
        reg.add(EditorConfigOverride::new("*.rs").with_tab_size(4));
        reg.add(EditorConfigOverride::new("*.rs").with_insert_spaces(true));
        assert_eq!(reg.find_all("main.rs").len(), 2);
        assert_eq!(reg.find_all("main.py").len(), 0);
    }

    #[test]
    fn override_serde_roundtrip() {
        let ov = EditorConfigOverride::new("*.rs")
            .with_tab_size(4)
            .with_insert_spaces(true);
        let json = serde_json::to_string(&ov).unwrap();
        let ov2: EditorConfigOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(ov, ov2);
    }

    #[test]
    fn override_serde_skips_none() {
        let ov = EditorConfigOverride::new("*.md");
        let json = serde_json::to_string(&ov).unwrap();
        assert!(!json.contains("tab_size"));
    }

    #[test]
    fn test_cursor_style_all() {
        assert_eq!(CursorStyle::all().len(), 6);
    }

    #[test]
    fn test_cursor_style_from_name() {
        assert_eq!(CursorStyle::from_name("block"), Some(CursorStyle::Block));
        assert_eq!(CursorStyle::from_name("line-thin"), Some(CursorStyle::LineThin));
        assert_eq!(CursorStyle::from_name("bogus"), None);
    }

    #[test]
    fn test_cursor_style_display() {
        assert_eq!(format!("{}", CursorStyle::Block), "block");
        assert_eq!(format!("{}", CursorStyle::LineThin), "line-thin");
    }

    #[test]
    fn test_cursor_style_cursor_char() {
        assert_eq!(CursorStyle::Block.cursor_char(), '█');
        assert_eq!(CursorStyle::Line.cursor_char(), '│');
    }

    #[test]
    fn test_word_wrap_all_and_enabled() {
        assert_eq!(WordWrap::all().len(), 5);
        assert!(WordWrap::On.is_enabled());
        assert!(!WordWrap::Off.is_enabled());
        assert!(!WordWrap::Inherit.is_enabled());
    }

    #[test]
    fn test_word_wrap_display() {
        assert_eq!(format!("{}", WordWrap::Bounded), "bounded");
    }

    #[test]
    fn test_line_numbers_type_all() {
        assert_eq!(LineNumbersType::all().len(), 4);
    }

    #[test]
    fn test_line_numbers_type_visible() {
        assert!(LineNumbersType::On.is_visible());
        assert!(!LineNumbersType::Off.is_visible());
    }

    #[test]
    fn test_diff_editor_options_same() {
        let a = EditorOptions::default();
        let b = EditorOptions::default();
        assert!(diff_editor_options(&a, &b).is_empty());
    }

    #[test]
    fn test_diff_editor_options_changed() {
        let a = EditorOptions::default();
        let mut b = EditorOptions::default();
        b.font_size = 20;
        b.tab_size = 2;
        let changes = diff_editor_options(&a, &b);
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| c.field == "font_size"));
        assert!(format!("{}", changes[0]).contains("->"));
    }

    // --- new tests ---

    #[test]
    fn profile_manager_add_activate() {
        let mut mgr = ProfileManager::new();
        mgr.add(EditorConfigProfile::new("default", EditorOptions::default()).with_default(true));
        mgr.add(EditorConfigProfile::new("minimal", EditorOptions::default()));
        assert_eq!(mgr.list().len(), 2);
        assert_eq!(mgr.active().unwrap().name, "default");
        assert!(mgr.activate("minimal"));
        assert_eq!(mgr.active().unwrap().name, "minimal");
        assert!(!mgr.activate("nonexistent"));
    }

    #[test]
    fn profile_manager_remove() {
        let mut mgr = ProfileManager::new();
        mgr.add(EditorConfigProfile::new("a", EditorOptions::default()));
        mgr.add(EditorConfigProfile::new("b", EditorOptions::default()));
        mgr.activate("a");
        assert!(mgr.remove("a"));
        assert_eq!(mgr.active().unwrap().name, "b");
    }

    #[test]
    fn profile_manager_clone_profile() {
        let mut mgr = ProfileManager::new();
        mgr.add(EditorConfigProfile::new("src", EditorOptions::default()));
        assert!(mgr.clone_profile("src", "dst"));
        assert_eq!(mgr.list().len(), 2);
        assert!(!mgr.clone_profile("src", "dst"));
        assert!(!mgr.clone_profile("nope", "other"));
    }

    #[test]
    fn config_merger_override() {
        let base = EditorOptions::default();
        let mut over = EditorOptions::default();
        over.tab_size = 2;
        over.font_size = 20;
        let merged = EditorConfigMerger::merge(&base, &over);
        assert_eq!(merged.tab_size, 2);
        assert_eq!(merged.font_size, 20);
        assert_eq!(merged.word_wrap, base.word_wrap);
    }

    #[test]
    fn config_validator_valid() {
        let opts = EditorOptions::default();
        assert!(ConfigValidator::validate(&opts).is_empty());
    }

    #[test]
    fn config_validator_invalid() {
        let mut opts = EditorOptions::default();
        opts.tab_size = 0;
        opts.font_size = 999;
        let errors = ConfigValidator::validate(&opts);
        assert!(errors.len() >= 2);
        assert!(errors.iter().any(|e| e.contains("tab_size")));
        assert!(errors.iter().any(|e| e.contains("font_size")));
    }

}
