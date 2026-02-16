//! Editor options and computed config

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
}
