//! Editor options and computed config

use std::collections::HashMap;
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

    /// Compute the effective line height; when `line_height` is 0 the font size
    /// scaled by a default factor (1.35) is used.
    pub fn effective_line_height(&self) -> u32 {
        if self.line_height > 0 {
            self.line_height
        } else {
            // 1.35× font_size, rounded
            ((self.font_size as f64) * 1.35).round() as u32
        }
    }

    /// Compute the effective cursor width; 0 means "use the cursor style default".
    pub fn effective_cursor_width(&self) -> u32 {
        if self.cursor_width > 0 {
            return self.cursor_width;
        }
        match self.cursor_style {
            CursorStyle::Line => 2,
            CursorStyle::LineThin => 1,
            _ => 0,
        }
    }

    /// Resolve the diff word-wrap setting: if `Inherit`, fall back to the
    /// editor-level `word_wrap`.
    pub fn resolved_diff_word_wrap(&self) -> WordWrap {
        if self.diff_word_wrap == WordWrap::Inherit {
            self.word_wrap
        } else {
            self.diff_word_wrap
        }
    }

    /// Build a compact indentation description string, e.g. "Spaces: 4" or "Tabs".
    pub fn indentation_label(&self) -> String {
        if self.insert_spaces {
            format!("Spaces: {}", self.tab_size)
        } else {
            "Tabs".to_string()
        }
    }

    /// Returns `true` when any automatic formatting feature is enabled.
    pub fn has_auto_format(&self) -> bool {
        self.format_on_paste || self.format_on_type || self.format_on_save
    }

    /// Count how many boolean "feature" flags are enabled.
    pub fn enabled_feature_count(&self) -> usize {
        let flags: &[bool] = &[
            self.insert_spaces,
            self.detect_indentation,
            self.trim_auto_whitespace,
            self.minimap_enabled,
            self.scroll_beyond_last_line,
            self.smooth_scrolling,
            self.render_control_characters,
            self.format_on_paste,
            self.format_on_type,
            self.format_on_save,
            self.suggest_on_trigger_characters,
            self.quick_suggestions,
            self.bracket_pair_colorization,
            self.guides_indentation,
            self.guides_bracket_pairs,
            self.folding_enabled,
            self.links,
            self.sticky_scroll_enabled,
        ];
        flags.iter().filter(|&&v| v).count()
    }

    /// Clamp numeric fields to sane ranges in-place, returning the number of
    /// fields that were adjusted.
    pub fn clamp_values(&mut self) -> usize {
        let mut adjusted = 0usize;
        macro_rules! clamp {
            ($field:ident, $min:expr, $max:expr) => {
                let clamped = self.$field.clamp($min, $max);
                if clamped != self.$field {
                    self.$field = clamped;
                    adjusted += 1;
                }
            };
        }
        clamp!(tab_size, 1, 16);
        clamp!(word_wrap_column, 1, 10_000);
        clamp!(cursor_width, 0, 10);
        clamp!(font_size, 1, 200);
        clamp!(line_height, 0, 200);
        clamp!(sticky_scroll_max_line_count, 0, 50);
        adjusted
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

    /// Returns `true` when at least one field is set to a non-`None` value.
    pub fn has_overrides(&self) -> bool {
        self.tab_size.is_some()
            || self.insert_spaces.is_some()
            || self.word_wrap.is_some()
            || self.rulers.is_some()
            || self.trim_trailing_whitespace.is_some()
            || self.insert_final_newline.is_some()
    }

    /// Count how many fields are overridden (non-`None`).
    pub fn override_count(&self) -> usize {
        [
            self.tab_size.is_some(),
            self.insert_spaces.is_some(),
            self.word_wrap.is_some(),
            self.rulers.is_some(),
            self.trim_trailing_whitespace.is_some(),
            self.insert_final_newline.is_some(),
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }

    /// Apply this override onto a mutable `EditorOptions`, touching only
    /// the fields that are `Some`.
    pub fn apply_to(&self, opts: &mut EditorOptions) {
        if let Some(ts) = self.tab_size {
            opts.tab_size = ts;
        }
        if let Some(is) = self.insert_spaces {
            opts.insert_spaces = is;
        }
        if let Some(ww) = self.word_wrap {
            opts.word_wrap = ww;
        }
        if let Some(ref r) = self.rulers {
            opts.rulers = r.clone();
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

    /// Remove all overrides whose pattern equals the given pattern.
    /// Returns how many were removed.
    pub fn remove_by_pattern(&mut self, pattern: &str) -> usize {
        let before = self.overrides.len();
        self.overrides.retain(|o| o.pattern != pattern);
        before - self.overrides.len()
    }

    /// Apply the *first* matching override to the given options.
    /// Returns `true` if an override was applied.
    pub fn apply_first(&self, filename: &str, opts: &mut EditorOptions) -> bool {
        if let Some(ov) = self.find(filename) {
            ov.apply_to(opts);
            true
        } else {
            false
        }
    }

    /// List all unique patterns registered.
    pub fn patterns(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for o in &self.overrides {
            if !seen.contains(&o.pattern.as_str()) {
                seen.push(o.pattern.as_str());
            }
        }
        seen
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

impl WordWrap {
    /// Parse from a string (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "on" => Some(Self::On),
            "wordwrapcolumn" | "word-wrap-column" => Some(Self::WordWrapColumn),
            "bounded" => Some(Self::Bounded),
            "inherit" => Some(Self::Inherit),
            _ => None,
        }
    }

    /// Returns `true` if wrapping uses a column limit.
    pub fn uses_column(&self) -> bool {
        matches!(self, WordWrap::WordWrapColumn | WordWrap::Bounded)
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

impl LineNumbersType {
    /// Parse from a string (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "on" => Some(Self::On),
            "relative" => Some(Self::Relative),
            "interval" => Some(Self::Interval),
            _ => None,
        }
    }

    /// Compute the displayed line number given the cursor line and the actual line.
    /// Both `cursor_line` and `line` are 1-based.
    pub fn render_line_number(&self, cursor_line: u32, line: u32, interval: u32) -> Option<u32> {
        match self {
            LineNumbersType::Off => None,
            LineNumbersType::On => Some(line),
            LineNumbersType::Relative => {
                if line == cursor_line {
                    Some(line)
                } else {
                    Some(line.abs_diff(cursor_line))
                }
            }
            LineNumbersType::Interval => {
                if line == cursor_line || interval == 0 {
                    Some(line)
                } else if line % interval == 0 {
                    Some(line)
                } else {
                    None
                }
            }
        }
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

// ---------------------------------------------------------------------------
// EditorConfig file parser
// ---------------------------------------------------------------------------

/// A single section from an `.editorconfig` file, with a glob pattern and
/// associated key-value properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConfigSection {
    pub glob_pattern: String,
    pub properties: HashMap<String, String>,
}

impl Default for EditorConfigSection {
    fn default() -> Self {
        Self {
            glob_pattern: String::from("*"),
            properties: HashMap::new(),
        }
    }
}

impl fmt::Display for EditorConfigSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.glob_pattern)?;
        let mut keys: Vec<&String> = self.properties.keys().collect();
        keys.sort();
        for k in keys {
            write!(f, " {}={}", k, self.properties[k])?;
        }
        Ok(())
    }
}

/// Parses INI-style `.editorconfig` file content into sections.
pub struct EditorConfigFileParser;

impl EditorConfigFileParser {
    /// Returns `true` if the content contains `root = true`.
    pub fn is_root(content: &str) -> bool {
        content.lines().any(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                return false;
            }
            let normalized: String = trimmed
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            normalized.eq_ignore_ascii_case("root=true")
        })
    }

    /// Parse `.editorconfig` content into a list of sections.
    pub fn parse(content: &str) -> Vec<EditorConfigSection> {
        let mut sections: Vec<EditorConfigSection> = Vec::new();
        let mut current: Option<EditorConfigSection> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with(';')
            {
                continue;
            }

            // Skip root declaration at top level.
            {
                let normalized: String =
                    trimmed.chars().filter(|c| !c.is_whitespace()).collect();
                if normalized.eq_ignore_ascii_case("root=true") {
                    continue;
                }
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Some(sec) = current.take() {
                    sections.push(sec);
                }
                let pattern = trimmed[1..trimmed.len() - 1].trim().to_string();
                current = Some(EditorConfigSection {
                    glob_pattern: pattern,
                    properties: HashMap::new(),
                });
            } else if let Some(ref mut sec) = current {
                if let Some((key, value)) = trimmed.split_once('=') {
                    sec.properties
                        .insert(key.trim().to_lowercase(), value.trim().to_string());
                }
            }
        }

        if let Some(sec) = current {
            sections.push(sec);
        }

        sections
    }
}

// ---------------------------------------------------------------------------
// EditorConfig inheritance
// ---------------------------------------------------------------------------

/// Manages `.editorconfig` files across a directory hierarchy and resolves
/// the effective properties for a given file path by merging from the
/// root-most directory to the leaf.
#[derive(Debug, Clone, Default)]
pub struct EditorConfigInheritance {
    configs: Vec<(String, Vec<EditorConfigSection>)>,
}

impl EditorConfigInheritance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register sections found in `dir_path`.
    pub fn add_config(&mut self, dir_path: &str, sections: Vec<EditorConfigSection>) {
        let normalized = dir_path.trim_end_matches('/').to_string();
        self.configs.push((normalized, sections));
    }

    /// Resolve the effective properties for `file_path` by merging configs
    /// from the shortest (root-most) matching directory to the longest.
    pub fn resolve(&self, file_path: &str) -> HashMap<String, String> {
        let mut matching: Vec<&(String, Vec<EditorConfigSection>)> = self
            .configs
            .iter()
            .filter(|(dir, _)| file_path.starts_with(dir.as_str()))
            .collect();

        // Sort by directory depth (shortest first = root-most).
        matching.sort_by_key(|(dir, _)| dir.len());

        let mut result = HashMap::new();
        let file_name = file_path.rsplit('/').next().unwrap_or(file_path);

        for (_, sections) in matching {
            for sec in sections {
                if Self::glob_matches(&sec.glob_pattern, file_name) {
                    for (k, v) in &sec.properties {
                        result.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        result
    }

    pub fn config_count(&self) -> usize {
        self.configs.len()
    }

    /// Simplified glob matching: supports `*` as a prefix/suffix wildcard.
    fn glob_matches(pattern: &str, file_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return file_name.ends_with(suffix);
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return file_name.starts_with(prefix);
        }
        pattern == file_name
    }
}

impl fmt::Display for EditorConfigInheritance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EditorConfigInheritance({} configs)", self.configs.len())
    }
}

// ---------------------------------------------------------------------------
// EditorConfig overrides
// ---------------------------------------------------------------------------

/// Workspace-level overrides that take precedence over `.editorconfig` values.
#[derive(Debug, Clone, Default)]
pub struct EditorConfigOverrides {
    overrides: HashMap<String, String>,
}

impl EditorConfigOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_override(&mut self, key: &str, value: &str) {
        self.overrides.insert(key.to_string(), value.to_string());
    }

    pub fn get_override(&self, key: &str) -> Option<&str> {
        self.overrides.get(key).map(|s| s.as_str())
    }

    pub fn remove_override(&mut self, key: &str) -> bool {
        self.overrides.remove(key).is_some()
    }

    pub fn overrides(&self) -> &HashMap<String, String> {
        &self.overrides
    }

    /// Merge workspace overrides on top of a base property map.
    pub fn merge_with(&self, base: &HashMap<String, String>) -> HashMap<String, String> {
        let mut merged = base.clone();
        for (k, v) in &self.overrides {
            merged.insert(k.clone(), v.clone());
        }
        merged
    }
}

impl fmt::Display for EditorConfigOverrides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EditorConfigOverrides({} entries)", self.overrides.len())
    }
}

// ---------------------------------------------------------------------------
// EditorConfig status indicator
// ---------------------------------------------------------------------------

/// Tracks the active `.editorconfig` properties for the currently focused file.
#[derive(Debug, Clone, Default)]
pub struct EditorConfigStatusIndicator {
    active_file: Option<String>,
    properties: HashMap<String, String>,
}

impl EditorConfigStatusIndicator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_active_config(&mut self, file_path: &str, properties: HashMap<String, String>) {
        self.active_file = Some(file_path.to_string());
        self.properties = properties;
    }

    pub fn active_file(&self) -> Option<&str> {
        self.active_file.as_deref()
    }

    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// One-line summary such as `"indent_size=4, tab_width=4"`.
    pub fn summary(&self) -> String {
        if self.properties.is_empty() {
            return String::from("no active config");
        }
        let mut keys: Vec<&String> = self.properties.keys().collect();
        keys.sort();
        keys.iter()
            .map(|k| format!("{}={}", k, self.properties[*k]))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for EditorConfigStatusIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.active_file {
            Some(path) => write!(f, "Active: {} ({})", path, self.summary()),
            None => write!(f, "No active config"),
        }
    }
}


// ---------------------------------------------------------------------------
// vsedit-editor-config: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConfigXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl EditorConfigXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for EditorConfigXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct EditorConfigXRegistry {
    entries: Vec<EditorConfigXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl EditorConfigXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: EditorConfigXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&EditorConfigXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut EditorConfigXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<EditorConfigXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&EditorConfigXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&EditorConfigXConfig> {
        let mut sorted: Vec<&EditorConfigXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&EditorConfigXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> EditorConfigXIterator<'_> {
        EditorConfigXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct EditorConfigXIterator<'a> {
    inner: std::slice::Iter<'a, EditorConfigXConfig>,
}

impl<'a> Iterator for EditorConfigXIterator<'a> {
    type Item = &'a EditorConfigXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct EditorConfigXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl EditorConfigXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct EditorConfigXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl EditorConfigXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &EditorConfigXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &EditorConfigXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &EditorConfigXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for EditorConfigXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct EditorConfigXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl EditorConfigXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &EditorConfigXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &EditorConfigXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for EditorConfigXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// Editor configuration resolution — extended utilities (ym)
// ---------------------------------------------------------------------------

/// Metric accumulator for editor_cfg operations.
#[derive(Debug, Clone)]
pub struct YmMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YmMetrics {
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

/// Sliding-window rate counter for editor_cfg.
#[derive(Debug, Clone)]
pub struct YmRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YmRateWindow {
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

/// A small LRU-style cache for editor_cfg lookups.
#[derive(Debug, Clone)]
pub struct YmLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YmLruCache {
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
// xa_ extended helpers for editor_config
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEditorConfigRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEditorConfigRingBuf {
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
pub struct XaEditorConfigCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEditorConfigCounter {
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

impl Default for XaEditorConfigCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 34
// ---------------------------------------------------------------------------

/// Generic object pool `Xc34Pool<T>`.
pub struct Xc34Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc34Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc34PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc34Pool<T> {
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
    pub fn stats(&self) -> Xc34PoolStats {
        Xc34PoolStats {
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

impl<T> Default for Xc34Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc34Scheduler`.
pub struct Xc34Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc34Scheduler {
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

impl Default for Xc34Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_34 hash for the given byte slice.
pub fn xc_34_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_34 convention.
pub fn xc_34_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_92 deepening: state machine + event bus ---

/// States for the Xd92 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd92State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd92State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd92Transition {
    pub from: Xd92State,
    pub to: Xd92State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd92StateMachine {
    current: Xd92State,
    history: Vec<Xd92Transition>,
    step_counter: usize,
}

impl Xd92StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd92State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd92State {
        self.current
    }

    pub fn history(&self) -> &[Xd92Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd92State) -> Result<Xd92State, String> {
        let allowed = match (self.current, target) {
            (Xd92State::Idle, Xd92State::Running) => true,
            (Xd92State::Running, Xd92State::Paused) => true,
            (Xd92State::Running, Xd92State::Done) => true,
            (Xd92State::Paused, Xd92State::Running) => true,
            (Xd92State::Paused, Xd92State::Done) => true,
            (Xd92State::Done, Xd92State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_92: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd92Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd92SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd92State> {
        let prefix = "Xd92SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd92State::Idle),
            "Running" => Some(Xd92State::Running),
            "Paused" => Some(Xd92State::Paused),
            "Done" => Some(Xd92State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd92State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd92 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd92Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd92Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd92HandlerFn = Box<dyn Fn(&Xd92Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd92EventBus {
    handlers: Vec<(usize, Option<String>, Xd92HandlerFn)>,
    next_id: usize,
    published: Vec<Xd92Event>,
}

impl Xd92EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd92Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd92Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd92Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd92Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_13: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg13Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg13Graph {
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

impl Default for Xg13Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_13: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg13Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg13Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg13Heap<T>) {
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

impl<T: Ord> Default for Xg13Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 33).
pub struct Xh33SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh33SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 75 as u64,
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

/// A compact bit set supporting boolean operations (variant 33).
pub struct Xh33BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh33BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 33).
pub struct Xi33Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi33Deque<T> {
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
pub struct Xi33Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi33Interval {
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

/// A simple interval tree (variant 33).
pub struct Xi33IntervalTree {
    xi_intervals: Vec<Xi33Interval>,
}

impl Xi33IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi33Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi33Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi33Interval) -> Vec<&Xi33Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi33Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi33Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi33Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi33Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi33Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi33Interval> = Vec::new();
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

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn word_wrap_from_name() {
        assert_eq!(WordWrap::from_name("off"), Some(WordWrap::Off));
        assert_eq!(WordWrap::from_name("ON"), Some(WordWrap::On));
        assert_eq!(WordWrap::from_name("bounded"), Some(WordWrap::Bounded));
        assert_eq!(WordWrap::from_name("wordwrapcolumn"), Some(WordWrap::WordWrapColumn));
        assert_eq!(WordWrap::from_name("word-wrap-column"), Some(WordWrap::WordWrapColumn));
        assert_eq!(WordWrap::from_name("inherit"), Some(WordWrap::Inherit));
        assert_eq!(WordWrap::from_name("nope"), None);
    }

    #[test]
    fn word_wrap_uses_column() {
        assert!(WordWrap::WordWrapColumn.uses_column());
        assert!(WordWrap::Bounded.uses_column());
        assert!(!WordWrap::Off.uses_column());
        assert!(!WordWrap::On.uses_column());
        assert!(!WordWrap::Inherit.uses_column());
    }

    #[test]
    fn line_numbers_from_name() {
        assert_eq!(LineNumbersType::from_name("off"), Some(LineNumbersType::Off));
        assert_eq!(LineNumbersType::from_name("ON"), Some(LineNumbersType::On));
        assert_eq!(LineNumbersType::from_name("Relative"), Some(LineNumbersType::Relative));
        assert_eq!(LineNumbersType::from_name("interval"), Some(LineNumbersType::Interval));
        assert_eq!(LineNumbersType::from_name("bogus"), None);
    }

    #[test]
    fn line_numbers_render_off() {
        assert_eq!(LineNumbersType::Off.render_line_number(5, 1, 10), None);
    }

    #[test]
    fn line_numbers_render_on() {
        assert_eq!(LineNumbersType::On.render_line_number(5, 3, 10), Some(3));
    }

    #[test]
    fn line_numbers_render_relative() {
        // At cursor line → absolute number
        assert_eq!(LineNumbersType::Relative.render_line_number(5, 5, 0), Some(5));
        // Away from cursor → distance
        assert_eq!(LineNumbersType::Relative.render_line_number(5, 8, 0), Some(3));
        assert_eq!(LineNumbersType::Relative.render_line_number(5, 2, 0), Some(3));
    }

    #[test]
    fn line_numbers_render_interval() {
        // Cursor line always shown
        assert_eq!(LineNumbersType::Interval.render_line_number(7, 7, 5), Some(7));
        // Multiples of interval shown
        assert_eq!(LineNumbersType::Interval.render_line_number(7, 10, 5), Some(10));
        // Non-multiples hidden
        assert_eq!(LineNumbersType::Interval.render_line_number(7, 3, 5), None);
    }

    #[test]
    fn effective_line_height_explicit() {
        let mut opts = EditorOptions::default();
        opts.line_height = 22;
        assert_eq!(opts.effective_line_height(), 22);
    }

    #[test]
    fn effective_line_height_auto() {
        let opts = EditorOptions::default(); // font_size 14, line_height 0
        // 14 * 1.35 = 18.9 → rounds to 19
        assert_eq!(opts.effective_line_height(), 19);
    }

    #[test]
    fn effective_cursor_width_defaults() {
        let mut opts = EditorOptions::default();
        opts.cursor_width = 0;
        opts.cursor_style = CursorStyle::Line;
        assert_eq!(opts.effective_cursor_width(), 2);
        opts.cursor_style = CursorStyle::LineThin;
        assert_eq!(opts.effective_cursor_width(), 1);
        opts.cursor_style = CursorStyle::Block;
        assert_eq!(opts.effective_cursor_width(), 0);
    }

    #[test]
    fn effective_cursor_width_explicit() {
        let mut opts = EditorOptions::default();
        opts.cursor_width = 5;
        assert_eq!(opts.effective_cursor_width(), 5);
    }

    #[test]
    fn resolved_diff_word_wrap_inherit() {
        let mut opts = EditorOptions::default();
        opts.word_wrap = WordWrap::On;
        opts.diff_word_wrap = WordWrap::Inherit;
        assert_eq!(opts.resolved_diff_word_wrap(), WordWrap::On);
    }

    #[test]
    fn resolved_diff_word_wrap_explicit() {
        let mut opts = EditorOptions::default();
        opts.word_wrap = WordWrap::Off;
        opts.diff_word_wrap = WordWrap::Bounded;
        assert_eq!(opts.resolved_diff_word_wrap(), WordWrap::Bounded);
    }

    #[test]
    fn indentation_label_spaces() {
        let opts = EditorOptions::default();
        assert_eq!(opts.indentation_label(), "Spaces: 4");
    }

    #[test]
    fn indentation_label_tabs() {
        let mut opts = EditorOptions::default();
        opts.insert_spaces = false;
        assert_eq!(opts.indentation_label(), "Tabs");
    }

    #[test]
    fn has_auto_format_none() {
        let opts = EditorOptions::default();
        assert!(!opts.has_auto_format());
    }

    #[test]
    fn has_auto_format_some() {
        let mut opts = EditorOptions::default();
        opts.format_on_save = true;
        assert!(opts.has_auto_format());
    }

    #[test]
    fn enabled_feature_count_default() {
        let opts = EditorOptions::default();
        // The default has many booleans enabled; just verify count > 0
        assert!(opts.enabled_feature_count() > 5);
    }

    #[test]
    fn clamp_values_no_change() {
        let mut opts = EditorOptions::default();
        assert_eq!(opts.clamp_values(), 0);
    }

    #[test]
    fn clamp_values_adjusts() {
        let mut opts = EditorOptions::default();
        opts.tab_size = 0;
        opts.font_size = 999;
        opts.word_wrap_column = 0;
        let adjusted = opts.clamp_values();
        assert_eq!(adjusted, 3);
        assert_eq!(opts.tab_size, 1);
        assert_eq!(opts.font_size, 200);
        assert_eq!(opts.word_wrap_column, 1);
    }

    #[test]
    fn override_has_overrides_empty() {
        let ov = EditorConfigOverride::new("*.rs");
        assert!(!ov.has_overrides());
        assert_eq!(ov.override_count(), 0);
    }

    #[test]
    fn override_has_overrides_some() {
        let ov = EditorConfigOverride::new("*.rs")
            .with_tab_size(2)
            .with_insert_spaces(true);
        assert!(ov.has_overrides());
        assert_eq!(ov.override_count(), 2);
    }

    #[test]
    fn override_apply_to() {
        let ov = EditorConfigOverride::new("*.py")
            .with_tab_size(2)
            .with_insert_spaces(true)
            .with_word_wrap(WordWrap::On)
            .with_rulers(vec![79]);
        let mut opts = EditorOptions::default();
        ov.apply_to(&mut opts);
        assert_eq!(opts.tab_size, 2);
        assert!(opts.insert_spaces);
        assert_eq!(opts.word_wrap, WordWrap::On);
        assert_eq!(opts.rulers, vec![79]);
    }

    #[test]
    fn registry_remove_by_pattern() {
        let mut reg = OverrideRegistry::new();
        reg.add(EditorConfigOverride::new("*.rs").with_tab_size(4));
        reg.add(EditorConfigOverride::new("*.rs").with_insert_spaces(true));
        reg.add(EditorConfigOverride::new("*.py").with_tab_size(2));
        assert_eq!(reg.remove_by_pattern("*.rs"), 2);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_apply_first() {
        let mut reg = OverrideRegistry::new();
        reg.add(EditorConfigOverride::new("*.rs").with_tab_size(2));
        let mut opts = EditorOptions::default();
        assert!(reg.apply_first("main.rs", &mut opts));
        assert_eq!(opts.tab_size, 2);
        assert!(!reg.apply_first("main.js", &mut opts));
    }

    #[test]
    fn registry_patterns() {
        let mut reg = OverrideRegistry::new();
        reg.add(EditorConfigOverride::new("*.rs"));
        reg.add(EditorConfigOverride::new("*.rs"));
        reg.add(EditorConfigOverride::new("*.py"));
        let pats = reg.patterns();
        assert_eq!(pats, vec!["*.rs", "*.py"]);
    }

    // -------------------------------------------------------------------
    // EditorConfigFileParser tests
    // -------------------------------------------------------------------

    #[test]
    fn parse_editorconfig_sections() {
        let content = "\
root = true

[*.rs]
indent_style = space
indent_size = 4

[Makefile]
indent_style = tab
";
        let sections = EditorConfigFileParser::parse(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].glob_pattern, "*.rs");
        assert_eq!(sections[0].properties.get("indent_style").unwrap(), "space");
        assert_eq!(sections[0].properties.get("indent_size").unwrap(), "4");
        assert_eq!(sections[1].glob_pattern, "Makefile");
        assert_eq!(sections[1].properties.get("indent_style").unwrap(), "tab");
    }

    #[test]
    fn parse_is_root() {
        assert!(EditorConfigFileParser::is_root("root = true\n[*]\nindent_size=2"));
        assert!(EditorConfigFileParser::is_root("  Root = True  "));
        assert!(!EditorConfigFileParser::is_root("[*]\nindent_size=2"));
        assert!(!EditorConfigFileParser::is_root("# root = true"));
    }

    #[test]
    fn parse_skips_comments() {
        let content = "\
[*.py]
# this is a comment
; so is this
indent_size = 2
";
        let sections = EditorConfigFileParser::parse(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].properties.len(), 1);
    }

    #[test]
    fn editorconfig_section_display() {
        let mut sec = EditorConfigSection::default();
        sec.glob_pattern = "*.rs".to_string();
        sec.properties.insert("indent_size".to_string(), "4".to_string());
        let display = format!("{}", sec);
        assert!(display.starts_with("[*.rs]"));
        assert!(display.contains("indent_size=4"));
    }

    // -------------------------------------------------------------------
    // EditorConfigInheritance tests
    // -------------------------------------------------------------------

    #[test]
    fn inheritance_resolve_merges_root_to_leaf() {
        let mut inh = EditorConfigInheritance::new();
        inh.add_config(
            "/project",
            vec![EditorConfigSection {
                glob_pattern: "*.rs".to_string(),
                properties: HashMap::from([
                    ("indent_size".to_string(), "4".to_string()),
                    ("charset".to_string(), "utf-8".to_string()),
                ]),
            }],
        );
        inh.add_config(
            "/project/src",
            vec![EditorConfigSection {
                glob_pattern: "*.rs".to_string(),
                properties: HashMap::from([
                    ("indent_size".to_string(), "2".to_string()),
                ]),
            }],
        );
        let resolved = inh.resolve("/project/src/main.rs");
        // Leaf overrides root for indent_size.
        assert_eq!(resolved.get("indent_size").unwrap(), "2");
        // Root's charset is inherited.
        assert_eq!(resolved.get("charset").unwrap(), "utf-8");
    }

    #[test]
    fn inheritance_config_count() {
        let mut inh = EditorConfigInheritance::new();
        assert_eq!(inh.config_count(), 0);
        inh.add_config("/a", vec![]);
        inh.add_config("/b", vec![]);
        assert_eq!(inh.config_count(), 2);
    }

    #[test]
    fn inheritance_display() {
        let inh = EditorConfigInheritance::new();
        assert_eq!(format!("{}", inh), "EditorConfigInheritance(0 configs)");
    }

    // -------------------------------------------------------------------
    // EditorConfigOverrides tests
    // -------------------------------------------------------------------

    #[test]
    fn overrides_set_get_remove() {
        let mut ov = EditorConfigOverrides::new();
        ov.set_override("indent_size", "2");
        assert_eq!(ov.get_override("indent_size"), Some("2"));
        assert!(ov.remove_override("indent_size"));
        assert_eq!(ov.get_override("indent_size"), None);
        assert!(!ov.remove_override("indent_size"));
    }

    #[test]
    fn overrides_merge_with() {
        let mut ov = EditorConfigOverrides::new();
        ov.set_override("indent_size", "2");
        ov.set_override("charset", "utf-16");

        let mut base = HashMap::new();
        base.insert("indent_size".to_string(), "4".to_string());
        base.insert("tab_width".to_string(), "4".to_string());

        let merged = ov.merge_with(&base);
        assert_eq!(merged.get("indent_size").unwrap(), "2"); // overridden
        assert_eq!(merged.get("tab_width").unwrap(), "4");   // from base
        assert_eq!(merged.get("charset").unwrap(), "utf-16"); // from overrides
    }

    #[test]
    fn overrides_display() {
        let ov = EditorConfigOverrides::new();
        assert_eq!(format!("{}", ov), "EditorConfigOverrides(0 entries)");
    }

    // -------------------------------------------------------------------
    // EditorConfigStatusIndicator tests
    // -------------------------------------------------------------------

    #[test]
    fn status_indicator_summary() {
        let mut si = EditorConfigStatusIndicator::new();
        assert_eq!(si.summary(), "no active config");
        assert_eq!(si.active_file(), None);

        let mut props = HashMap::new();
        props.insert("indent_size".to_string(), "4".to_string());
        props.insert("tab_width".to_string(), "4".to_string());
        si.set_active_config("src/main.rs", props);

        assert_eq!(si.active_file(), Some("src/main.rs"));
        assert_eq!(si.get_property("indent_size"), Some("4"));
        assert_eq!(si.summary(), "indent_size=4, tab_width=4");
    }

    #[test]
    fn status_indicator_display() {
        let mut si = EditorConfigStatusIndicator::new();
        assert_eq!(format!("{}", si), "No active config");
        si.set_active_config("a.rs", HashMap::from([("k".to_string(), "v".to_string())]));
        let display = format!("{}", si);
        assert!(display.contains("a.rs"));
        assert!(display.contains("k=v"));
    }


    #[test]
    fn editorConfig_x_config_new() {
        let c = EditorConfigXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn editorConfig_x_config_builder() {
        let c = EditorConfigXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn editorConfig_x_config_display() {
        let c = EditorConfigXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn editorConfig_x_registry_insert_get() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn editorConfig_x_registry_duplicate() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a")).unwrap();
        assert!(reg.insert(EditorConfigXConfig::new("a")).is_err());
    }

    #[test]
    fn editorConfig_x_registry_remove() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a")).unwrap();
        reg.insert(EditorConfigXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn editorConfig_x_registry_active_entries() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a")).unwrap();
        reg.insert(EditorConfigXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn editorConfig_x_registry_by_weight() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(EditorConfigXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn editorConfig_x_registry_tags() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(EditorConfigXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn editorConfig_x_registry_total_weight() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(EditorConfigXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn editorConfig_x_registry_iterator() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a")).unwrap();
        reg.insert(EditorConfigXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn editorConfig_x_cache_put_get() {
        let mut cache = EditorConfigXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn editorConfig_x_cache_eviction() {
        let mut cache = EditorConfigXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn editorConfig_x_cache_lru_order() {
        let mut cache = EditorConfigXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn editorConfig_x_cache_most_least_recent() {
        let mut cache = EditorConfigXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn editorConfig_x_formatter_entry() {
        let e = EditorConfigXConfig::new("k").with_value("v");
        let fmt = EditorConfigXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn editorConfig_x_formatter_summary() {
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("a").with_weight(5)).unwrap();
        let fmt = EditorConfigXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn editorConfig_x_validator_valid() {
        let v = EditorConfigXValidator::new();
        let c = EditorConfigXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn editorConfig_x_validator_empty_key() {
        let v = EditorConfigXValidator::new();
        let c = EditorConfigXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorConfig_x_validator_require_value() {
        let v = EditorConfigXValidator::new().require_value(true);
        let c = EditorConfigXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorConfig_x_validator_allowed_tags() {
        let v = EditorConfigXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = EditorConfigXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorConfig_x_validator_validate_all() {
        let v = EditorConfigXValidator::new();
        let mut reg = EditorConfigXRegistry::new();
        reg.insert(EditorConfigXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn ym_metrics_empty() {
        let m = YmMetrics::new("editor_cfg");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ym_metrics_record_and_mean() {
        let mut m = YmMetrics::new("editor_cfg");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ym_metrics_min_max() {
        let mut m = YmMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ym_metrics_variance_and_std() {
        let mut m = YmMetrics::new("v");
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
    fn ym_metrics_percentile() {
        let mut m = YmMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn ym_metrics_merge() {
        let mut a = YmMetrics::new("a");
        a.record(1.0);
        let mut b = YmMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn ym_metrics_reset() {
        let mut m = YmMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn ym_rate_window_empty() {
        let rw = YmRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn ym_rate_window_tick_and_rate() {
        let mut rw = YmRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn ym_lru_cache_basic() {
        let mut c = YmLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn ym_lru_cache_contains_and_keys() {
        let mut c = YmLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn ym_lru_cache_remove() {
        let mut c = YmLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn ym_metrics_sum() {
        let mut m = YmMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ym_metrics_label() {
        let m = YmMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn ym_lru_cache_clear() {
        let mut c = YmLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for editor_config
    #[test]
    fn xa_editor_config_ring_new() {
        let rb = super::XaEditorConfigRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_editor_config_ring_push_len() {
        let mut rb = super::XaEditorConfigRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_editor_config_ring_wrap() {
        let mut rb = super::XaEditorConfigRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_editor_config_ring_mean_empty() {
        let rb = super::XaEditorConfigRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_editor_config_ring_mean_values() {
        let mut rb = super::XaEditorConfigRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_editor_config_ring_min_max() {
        let mut rb = super::XaEditorConfigRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_editor_config_ring_iter() {
        let mut rb = super::XaEditorConfigRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_editor_config_counter_new() {
        let c = super::XaEditorConfigCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_config_counter_inc() {
        let mut c = super::XaEditorConfigCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_editor_config_counter_inc_by() {
        let mut c = super::XaEditorConfigCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_editor_config_counter_reset() {
        let mut c = super::XaEditorConfigCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_editor_config_counter_clear() {
        let mut c = super::XaEditorConfigCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_config_counter_default() {
        let c = super::XaEditorConfigCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 34 ----

    #[test]
    fn xc_34_pool_new_empty() {
        let pool: super::Xc34Pool<i32> = super::Xc34Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_34_pool_release_acquire() {
        let mut pool = super::Xc34Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_34_pool_acquire_empty() {
        let mut pool: super::Xc34Pool<i32> = super::Xc34Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_34_pool_full() {
        let mut pool = super::Xc34Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_34_pool_drain() {
        let mut pool = super::Xc34Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_34_pool_stats() {
        let mut pool = super::Xc34Pool::new(8);
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
    fn xc_34_pool_clear() {
        let mut pool = super::Xc34Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_34_pool_shrink() {
        let mut pool = super::Xc34Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_34_pool_default() {
        let pool: super::Xc34Pool<String> = super::Xc34Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_34_pool_extend() {
        let mut pool = super::Xc34Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_34_pool_retain() {
        let mut pool = super::Xc34Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_34_scheduler_round_robin() {
        let mut sched = super::Xc34Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_34_scheduler_empty() {
        let mut sched = super::Xc34Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_34_scheduler_reset() {
        let mut sched = super::Xc34Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_34_scheduler_add_remove() {
        let mut sched = super::Xc34Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_34_scheduler_targets() {
        let sched = super::Xc34Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_34_hash_empty() {
        assert_eq!(super::xc_34_hash(b""), 5381);
    }

    #[test]
    fn xc_34_hash_data() {
        let h = super::xc_34_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_34_hash(b"hello"), h);
    }

    #[test]
    fn xc_34_reverse_str() {
        assert_eq!(super::xc_34_reverse("abc"), "cba");
        assert_eq!(super::xc_34_reverse(""), "");
    }


    // --- xd_92 deepening tests ---

    #[test]
    fn xd_92_sm_initial_state() {
        let sm = Xd92StateMachine::new();
        assert_eq!(sm.current_state(), Xd92State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_92_sm_valid_idle_to_running() {
        let mut sm = Xd92StateMachine::new();
        assert!(sm.transition(Xd92State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd92State::Running);
    }

    #[test]
    fn xd_92_sm_valid_running_to_paused() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        assert!(sm.transition(Xd92State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd92State::Paused);
    }

    #[test]
    fn xd_92_sm_valid_running_to_done() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        assert!(sm.transition(Xd92State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd92State::Done);
    }

    #[test]
    fn xd_92_sm_valid_paused_to_running() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        sm.transition(Xd92State::Paused).unwrap();
        assert!(sm.transition(Xd92State::Running).is_ok());
    }

    #[test]
    fn xd_92_sm_valid_done_to_idle() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        sm.transition(Xd92State::Done).unwrap();
        assert!(sm.transition(Xd92State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd92State::Idle);
    }

    #[test]
    fn xd_92_sm_invalid_idle_to_done() {
        let mut sm = Xd92StateMachine::new();
        assert!(sm.transition(Xd92State::Done).is_err());
    }

    #[test]
    fn xd_92_sm_invalid_idle_to_paused() {
        let mut sm = Xd92StateMachine::new();
        assert!(sm.transition(Xd92State::Paused).is_err());
    }

    #[test]
    fn xd_92_sm_history_tracking() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        sm.transition(Xd92State::Paused).unwrap();
        sm.transition(Xd92State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd92State::Idle);
        assert_eq!(sm.history()[0].to, Xd92State::Running);
        assert_eq!(sm.history()[1].from, Xd92State::Running);
        assert_eq!(sm.history()[2].to, Xd92State::Done);
    }

    #[test]
    fn xd_92_sm_serialize_deserialize() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd92StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd92State::Running));
    }

    #[test]
    fn xd_92_sm_deserialize_invalid() {
        assert_eq!(Xd92StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_92_sm_reset() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd92State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_92_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd92EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd92Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_92_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd92EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd92Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd92Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_92_bus_unsubscribe() {
        let mut bus = Xd92EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_92_event_kind_and_payload() {
        let e = Xd92Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd92Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_92_bus_clear_history() {
        let mut bus = Xd92EventBus::new();
        bus.publish(Xd92Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_92_sm_step_counter_increments() {
        let mut sm = Xd92StateMachine::new();
        sm.transition(Xd92State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd92State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_13 graph tests ------------------------------------------------

    #[test]
    fn xg_13_graph_empty() {
        let g = super::Xg13Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_13_graph_add_node() {
        let mut g = super::Xg13Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_13_graph_add_edge() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_13_graph_neighbors() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_13_graph_has_path() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_13_graph_self_path() {
        let g = super::Xg13Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_13_graph_topo_sort() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_13_graph_cycle_detect_false() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_13_graph_cycle_detect_true() {
        let mut g = super::Xg13Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_13 heap tests -------------------------------------------------

    #[test]
    fn xg_13_heap_empty() {
        let h: super::Xg13Heap<i32> = super::Xg13Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_13_heap_push_pop() {
        let mut h = super::Xg13Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_13_heap_peek() {
        let mut h = super::Xg13Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_13_heap_drain_sorted() {
        let mut h = super::Xg13Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_13_heap_merge() {
        let mut a = super::Xg13Heap::new();
        let mut b = super::Xg13Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_13_heap_default() {
        let h: super::Xg13Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_13_graph_default() {
        let g: super::Xg13Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh33_skip_insert_contains() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh33_skip_remove() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh33_skip_len() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh33_skip_range_query() {
        let mut sl = super::Xh33SkipList::xh_new(4);
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
    fn xh33_skip_floor_ceiling() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh33_skip_rank() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh33_skip_empty() {
        let sl = super::Xh33SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh33_skip_duplicates() {
        let mut sl = super::Xh33SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh33_bitset_set_test() {
        let mut bs = super::Xh33BitSet::xh_new(256);
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
    fn xh33_bitset_clear_count() {
        let mut bs = super::Xh33BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh33_bitset_and_or_xor() {
        let mut a = super::Xh33BitSet::xh_new(128);
        let mut b = super::Xh33BitSet::xh_new(128);
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
    fn xh33_bitset_iter_ones() {
        let mut bs = super::Xh33BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh33_bitset_first_last() {
        let mut bs = super::Xh33BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh33_bitset_empty() {
        let bs = super::Xh33BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi33_deque_push_pop_back() {
        let mut dq = super::Xi33Deque::xi_new(4);
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
    fn xi33_deque_push_pop_front() {
        let mut dq = super::Xi33Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi33_deque_mixed_ops() {
        let mut dq = super::Xi33Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi33_deque_get_and_split() {
        let mut dq = super::Xi33Deque::xi_new(8);
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
    fn xi33_deque_rotate_left() {
        let mut dq = super::Xi33Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi33_deque_rotate_right() {
        let mut dq = super::Xi33Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi33_deque_grow() {
        let mut dq = super::Xi33Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi33_deque_empty() {
        let dq = super::Xi33Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi33_interval_tree_insert_query() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi33Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi33Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi33_interval_tree_overlap() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi33Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi33Interval::xi_new(12, 20));
        let q = super::Xi33Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi33_interval_tree_remove() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi33Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi33_interval_tree_gaps() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi33Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi33Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi33Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi33Interval::xi_new(8, 10));
    }

    #[test]
    fn xi33_interval_tree_merge() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi33Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi33Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi33Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi33Interval::xi_new(10, 15));
    }

    #[test]
    fn xi33_interval_tree_all() {
        let mut tree = super::Xi33IntervalTree::xi_new();
        tree.xi_insert(super::Xi33Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi33Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi33_interval_tree_empty() {
        let tree = super::Xi33IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi33_interval_tree_contains_point() {
        let iv = super::Xi33Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
