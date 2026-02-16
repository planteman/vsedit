//! Unicode-aware terminal text rendering.
//!
//! Equivalent to VS Code's text rendering pipeline, adapted for terminal output.
//! Handles wide characters, tab expansion, and control characters.

use std::fmt;

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Errors that can occur during text rendering operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// Tab size must be at least 1.
    InvalidTabSize(u32),
    /// The requested column is out of bounds.
    ColumnOutOfBounds { requested: usize, max: usize },
    /// The requested width is zero.
    ZeroWidth,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::InvalidTabSize(n) => write!(f, "invalid tab size: {} (must be >= 1)", n),
            RenderError::ColumnOutOfBounds { requested, max } => {
                write!(f, "column {} out of bounds (max {})", requested, max)
            }
            RenderError::ZeroWidth => write!(f, "width must be greater than zero"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Result of rendering a line for terminal display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    /// The display string (with tabs expanded, control chars replaced).
    pub text: String,
    /// Map from display column (0-based) to source offset (0-based byte offset).
    pub column_to_offset: Vec<usize>,
    /// Total display width.
    pub display_width: usize,
}

/// Render a text line for terminal display.
pub fn render_line(text: &str, tab_size: u32) -> RenderedLine {
    let mut result = String::with_capacity(text.len());
    let mut column_map: Vec<usize> = Vec::with_capacity(text.len());
    let mut display_col: usize = 0;

    for (byte_offset, ch) in text.char_indices() {
        match ch {
            '\t' => {
                let spaces = tab_size as usize - (display_col % tab_size as usize);
                for _ in 0..spaces {
                    result.push(' ');
                    column_map.push(byte_offset);
                    display_col += 1;
                }
            }
            '\r' | '\n' => {
                // Skip line endings
            }
            c if c.is_control() => {
                // Show control chars as Unicode control pictures (U+2400 range)
                let replacement = char::from_u32(0x2400 + c as u32).unwrap_or('?');
                result.push(replacement);
                column_map.push(byte_offset);
                display_col += 1;
            }
            c => {
                let width = UnicodeWidthChar::width(c).unwrap_or(0);
                result.push(c);
                column_map.push(byte_offset);
                display_col += 1;
                // Wide characters take 2 columns
                if width > 1 {
                    // Add padding column for wide char
                    column_map.push(byte_offset);
                    display_col += 1;
                }
            }
        }
    }

    RenderedLine {
        text: result,
        column_to_offset: column_map,
        display_width: display_col,
    }
}

/// Calculate the display width of a string.
pub fn display_width(text: &str, tab_size: u32) -> usize {
    let mut width: usize = 0;
    for ch in text.chars() {
        match ch {
            '\t' => {
                width += tab_size as usize - (width % tab_size as usize);
            }
            '\r' | '\n' => {}
            c if c.is_control() => {
                width += 1;
            }
            c => {
                width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }
    }
    width
}

/// Truncate a string to fit within a given display width.
pub fn truncate_to_width(text: &str, max_width: usize, tab_size: u32) -> String {
    let mut result = String::new();
    let mut width: usize = 0;

    for ch in text.chars() {
        let ch_width = match ch {
            '\t' => tab_size as usize - (width % tab_size as usize),
            '\r' | '\n' => 0,
            c if c.is_control() => 1,
            c => UnicodeWidthChar::width(c).unwrap_or(0),
        };

        if width + ch_width > max_width {
            if width < max_width {
                result.push('…');
            }
            break;
        }

        if ch == '\t' {
            for _ in 0..ch_width {
                result.push(' ');
            }
        } else {
            result.push(ch);
        }
        width += ch_width;
    }

    result
}

/// Render whitespace characters visually.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceRender {
    None,
    Boundary,
    Selection,
    Trailing,
    All,
}

/// Replace whitespace with visible characters based on render mode.
pub fn render_whitespace(text: &str, mode: WhitespaceRender) -> String {
    if mode == WhitespaceRender::None {
        return text.to_string();
    }

    let trimmed_len = text.trim_end().len();
    let mut result = String::with_capacity(text.len());

    for (i, ch) in text.char_indices() {
        let is_trailing = i >= trimmed_len;

        let should_render = match mode {
            WhitespaceRender::All => true,
            WhitespaceRender::Trailing => is_trailing,
            WhitespaceRender::Boundary => {
                // Render at word boundaries (consecutive spaces)
                if ch == ' ' {
                    let next = text[i + 1..].chars().next();
                    let prev = if i > 0 {
                        text[..i].chars().next_back()
                    } else {
                        None
                    };
                    prev == Some(' ') || next == Some(' ') || is_trailing
                } else {
                    ch == '\t'
                }
            }
            WhitespaceRender::Selection | WhitespaceRender::None => false,
        };

        if should_render && ch == ' ' {
            result.push('·');
        } else if should_render && ch == '\t' {
            result.push('→');
        } else {
            result.push(ch);
        }
    }

    result
}

impl fmt::Display for RenderedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl RenderedLine {
    /// Return the source byte offset for a given display column.
    pub fn offset_at_column(&self, col: usize) -> Result<usize, RenderError> {
        self.column_to_offset.get(col).copied().ok_or(RenderError::ColumnOutOfBounds {
            requested: col,
            max: self.column_to_offset.len().saturating_sub(1),
        })
    }

    /// Return a substring of the rendered text spanning the given display columns.
    pub fn slice_columns(&self, start: usize, end: usize) -> &str {
        let byte_start = start.min(self.text.len());
        let byte_end = end.min(self.text.len());
        &self.text[byte_start..byte_end]
    }

    /// True when the rendered line contains no visible characters.
    pub fn is_empty(&self) -> bool {
        self.display_width == 0
    }
}

/// Configuration for line rendering behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    /// Number of spaces per tab stop.
    pub tab_size: u32,
    /// Maximum display width (0 = unlimited).
    pub max_width: usize,
    /// How to render whitespace.
    pub whitespace_mode: WhitespaceRender,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            max_width: 0,
            whitespace_mode: WhitespaceRender::None,
        }
    }
}

impl RenderConfig {
    /// Create a new builder for `RenderConfig`.
    pub fn builder() -> RenderConfigBuilder {
        RenderConfigBuilder::default()
    }

    /// Validate the configuration, returning an error for invalid values.
    pub fn validate(&self) -> Result<(), RenderError> {
        if self.tab_size == 0 {
            return Err(RenderError::InvalidTabSize(0));
        }
        Ok(())
    }

    /// Render a line using this configuration.
    pub fn render(&self, text: &str) -> Result<RenderedLine, RenderError> {
        self.validate()?;
        let mut rendered = render_line(text, self.tab_size);
        if self.max_width > 0 && rendered.display_width > self.max_width {
            rendered.text = truncate_to_width(text, self.max_width, self.tab_size);
            rendered.display_width = display_width(&rendered.text, self.tab_size);
            rendered.column_to_offset.truncate(rendered.display_width);
        }
        if self.whitespace_mode != WhitespaceRender::None {
            rendered.text = render_whitespace(&rendered.text, self.whitespace_mode);
        }
        Ok(rendered)
    }
}

/// Builder for [`RenderConfig`].
#[derive(Debug, Clone, Default)]
pub struct RenderConfigBuilder {
    tab_size: Option<u32>,
    max_width: Option<usize>,
    whitespace_mode: Option<WhitespaceRender>,
}

impl RenderConfigBuilder {
    pub fn tab_size(mut self, size: u32) -> Self {
        self.tab_size = Some(size);
        self
    }

    pub fn max_width(mut self, width: usize) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn whitespace_mode(mut self, mode: WhitespaceRender) -> Self {
        self.whitespace_mode = Some(mode);
        self
    }

    /// Build the configuration, validating all values.
    pub fn build(self) -> Result<RenderConfig, RenderError> {
        let config = RenderConfig {
            tab_size: self.tab_size.unwrap_or(4),
            max_width: self.max_width.unwrap_or(0),
            whitespace_mode: self.whitespace_mode.unwrap_or(WhitespaceRender::None),
        };
        config.validate()?;
        Ok(config)
    }
}

/// Compute the display column for a given byte offset in a line.
pub fn byte_offset_to_column(text: &str, byte_offset: usize, tab_size: u32) -> usize {
    let mut col: usize = 0;
    for (i, ch) in text.char_indices() {
        if i >= byte_offset {
            break;
        }
        match ch {
            '\t' => {
                col += tab_size as usize - (col % tab_size as usize);
            }
            '\r' | '\n' => {}
            c if c.is_control() => {
                col += 1;
            }
            c => {
                col += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }
    }
    col
}

/// Compute the byte offset for a given display column in a line.
///
/// Returns `None` if the column exceeds the line's display width.
pub fn column_to_byte_offset(text: &str, target_col: usize, tab_size: u32) -> Option<usize> {
    let mut col: usize = 0;
    for (i, ch) in text.char_indices() {
        if col >= target_col {
            return Some(i);
        }
        match ch {
            '\t' => {
                col += tab_size as usize - (col % tab_size as usize);
            }
            '\r' | '\n' => {}
            c if c.is_control() => {
                col += 1;
            }
            c => {
                col += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }
    }
    if col >= target_col {
        Some(text.len())
    } else {
        None
    }
}

/// Pad (or truncate) a rendered string to exactly `width` display columns.
pub fn pad_to_width(text: &str, width: usize, tab_size: u32) -> String {
    let current = display_width(text, tab_size);
    if current >= width {
        truncate_to_width(text, width, tab_size)
    } else {
        let mut s = text.to_string();
        for _ in 0..(width - current) {
            s.push(' ');
        }
        s
    }
}

/// How text should be wrapped when it exceeds the available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWrapMode {
    /// No wrapping; lines may exceed the available width.
    None,
    /// Wrap at word boundaries (whitespace).
    Word,
    /// Wrap at any character boundary.
    Character,
}

/// Wrap `text` into lines that fit within `max_width` display columns.
///
/// Uses [`UnicodeWidthChar`] to measure character widths so that wide (CJK)
/// characters are handled correctly.  When `mode` is [`TextWrapMode::None`]
/// the input is returned as a single element.
pub fn wrap_text(text: &str, max_width: usize, mode: TextWrapMode) -> Vec<String> {
    if max_width == 0 || mode == TextWrapMode::None {
        return vec![text.to_string()];
    }

    match mode {
        TextWrapMode::None => vec![text.to_string()],
        TextWrapMode::Character => wrap_by_character(text, max_width),
        TextWrapMode::Word => wrap_by_word(text, max_width),
    }
}

fn wrap_by_character(text: &str, max_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut col: usize = 0;

    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + w > max_width && col > 0 {
            lines.push(current);
            current = String::new();
            col = 0;
        }
        current.push(ch);
        col += w;
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn wrap_by_word(text: &str, max_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut col: usize = 0;

    for word in text.split_inclusive(' ') {
        let word_width: usize = UnicodeWidthStr::width(word);
        if col + word_width > max_width && col > 0 {
            lines.push(current.trim_end().to_string());
            current = String::new();
            col = 0;
        }
        current.push_str(word);
        col += word_width;
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current.trim_end().to_string());
    }
    lines
}

/// Measurements of a text string for layout purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMeasurement {
    /// Display width in terminal columns (accounts for wide characters).
    pub visible_width: usize,
    /// Length in bytes.
    pub byte_length: usize,
    /// Number of Unicode scalar values.
    pub char_count: usize,
    /// Whether the text contains any characters wider than one column.
    pub contains_wide_chars: bool,
}

/// Measure a string, returning layout-relevant metrics.
pub fn measure_text(text: &str) -> TextMeasurement {
    let visible_width = UnicodeWidthStr::width(text);
    let byte_length = text.len();
    let char_count = text.chars().count();
    let contains_wide_chars = text
        .chars()
        .any(|c| UnicodeWidthChar::width(c).unwrap_or(0) > 1);

    TextMeasurement {
        visible_width,
        byte_length,
        char_count,
        contains_wide_chars,
    }
}

/// Truncate `text` to fit within `max_width` display columns, appending
/// `ellipsis` when truncation occurs.
///
/// Unlike [`truncate_to_width`] this function lets the caller choose the
/// ellipsis string (e.g. `"..."` or `"…"`).
pub fn truncate_with_ellipsis(text: &str, max_width: usize, ellipsis: &str) -> String {
    let text_width = UnicodeWidthStr::width(text);
    if text_width <= max_width {
        return text.to_string();
    }

    let ellipsis_width = UnicodeWidthStr::width(ellipsis);
    if max_width <= ellipsis_width {
        // Not enough room for even the ellipsis; return what fits.
        let mut result = String::new();
        let mut col: usize = 0;
        for ch in text.chars() {
            let w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if col + w > max_width {
                break;
            }
            result.push(ch);
            col += w;
        }
        return result;
    }

    let target = max_width - ellipsis_width;
    let mut result = String::new();
    let mut col: usize = 0;
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + w > target {
            break;
        }
        result.push(ch);
        col += w;
    }
    result.push_str(ellipsis);
    result
}

// ── Line truncation with configurable ellipsis ──

/// Truncate a line to fit within `max_width` display columns, adding an
/// ellipsis string when truncation occurs.  Unlike [`truncate_with_ellipsis`],
/// this function also handles tab expansion during measurement.
pub fn truncate_line_with_ellipsis(text: &str, max_width: usize, tab_size: u32, ellipsis: &str) -> String {
    let text_width = display_width(text, tab_size);
    if text_width <= max_width {
        return text.to_string();
    }
    let ell_width = UnicodeWidthStr::width(ellipsis);
    if max_width <= ell_width {
        return truncate_to_width(text, max_width, tab_size);
    }
    let target = max_width - ell_width;
    let mut result = truncate_to_width(text, target, tab_size);
    // Remove the built-in '…' if truncate_to_width appended one
    if result.ends_with('…') {
        result.pop();
    }
    result.push_str(ellipsis);
    result
}

// ── Bidirectional text hints ──

/// Hint about the dominant text direction of a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiHint {
    LeftToRight,
    RightToLeft,
    Neutral,
}

/// Determine a simple bidirectional hint for `text` by checking whether it
/// starts with a known RTL Unicode block character.
pub fn detect_bidi_hint(text: &str) -> BidiHint {
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        let cp = ch as u32;
        // Arabic (0600-06FF), Hebrew (0590-05FF), Arabic Supplement/Extended
        if (0x0590..=0x05FF).contains(&cp)
            || (0x0600..=0x06FF).contains(&cp)
            || (0xFB50..=0xFDFF).contains(&cp)
            || (0xFE70..=0xFEFF).contains(&cp)
        {
            return BidiHint::RightToLeft;
        }
        return BidiHint::LeftToRight;
    }
    BidiHint::Neutral
}

// ── Control character visualization ──

/// Replace ASCII control characters (0x00–0x1F except \t, \n, \r) with their
/// Unicode Control Pictures (U+2400 range) for display.
pub fn visualize_control_chars(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch != '\t' && ch != '\n' && ch != '\r' && ch.is_control() {
            let replacement = char::from_u32(0x2400 + ch as u32).unwrap_or('?');
            out.push(replacement);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Count the number of control characters in a string (excluding tab, LF, CR).
pub fn count_control_chars(text: &str) -> usize {
    text.chars()
        .filter(|&c| c != '\t' && c != '\n' && c != '\r' && c.is_control())
        .count()
}

/// Pad a string to `width` display columns using the given `fill` character.
/// If the string is already at least `width` columns, it is truncated instead.
pub fn pad_to_width_with_char(text: &str, width: usize, tab_size: u32, fill: char) -> String {
    let current = display_width(text, tab_size);
    if current >= width {
        truncate_to_width(text, width, tab_size)
    } else {
        let mut s = text.to_string();
        for _ in 0..(width - current) {
            s.push(fill);
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Tab expansion, centering, and column alignment
// ---------------------------------------------------------------------------

/// Expand all tab characters to spaces, aligning to tab stops.
///
/// Unlike `render_line`, this function returns a simple string without
/// column mapping metadata.
pub fn tab_expand(text: &str, tab_size: u32) -> Result<String, RenderError> {
    if tab_size == 0 {
        return Err(RenderError::InvalidTabSize(0));
    }
    let mut result = String::with_capacity(text.len());
    let mut col: usize = 0;
    for ch in text.chars() {
        if ch == '\t' {
            let spaces = tab_size as usize - (col % tab_size as usize);
            for _ in 0..spaces {
                result.push(' ');
            }
            col += spaces;
        } else {
            result.push(ch);
            col += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
    }
    Ok(result)
}

/// Center text within a given display width, padding with spaces.
///
/// If the text is wider than `width`, it is returned as-is.
pub fn center_text(text: &str, width: usize, tab_size: u32) -> String {
    let text_width = display_width(text, tab_size);
    if text_width >= width {
        return text.to_string();
    }
    let total_pad = width - text_width;
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    let mut result = String::with_capacity(width);
    for _ in 0..left_pad {
        result.push(' ');
    }
    result.push_str(text);
    for _ in 0..right_pad {
        result.push(' ');
    }
    result
}

/// Right-align text within a given display width, padding with spaces on the left.
pub fn right_align(text: &str, width: usize, tab_size: u32) -> String {
    let text_width = display_width(text, tab_size);
    if text_width >= width {
        return text.to_string();
    }
    let pad = width - text_width;
    let mut result = String::with_capacity(width);
    for _ in 0..pad {
        result.push(' ');
    }
    result.push_str(text);
    result
}

/// Count the number of visible lines after wrapping text to `max_width`.
pub fn visible_line_count(text: &str, max_width: usize, mode: TextWrapMode) -> usize {
    if max_width == 0 || mode == TextWrapMode::None {
        return 1;
    }
    wrap_text(text, max_width, mode).len()
}

/// Align a list of strings into columns with the given column separator.
///
/// Each inner Vec<String> represents a row. The function pads each column
/// to the width of the widest entry in that column.
pub fn column_align(rows: &[Vec<String>], separator: &str, tab_size: u32) -> Vec<String> {
    if rows.is_empty() {
        return Vec::new();
    }

    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; max_cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell, tab_size);
            if w > col_widths[i] {
                col_widths[i] = w;
            }
        }
    }

    rows.iter()
        .map(|row| {
            let mut parts: Vec<String> = Vec::new();
            for (i, cell) in row.iter().enumerate() {
                if i < row.len() - 1 {
                    parts.push(pad_to_width(cell, col_widths[i], tab_size));
                } else {
                    parts.push(cell.clone());
                }
            }
            parts.join(separator)
        })
        .collect()
}

/// Strip leading whitespace uniformly from all lines, preserving relative indentation.
///
/// Finds the minimum indentation across all non-empty lines and removes that
/// many columns from each line.
pub fn dedent(text: &str, tab_size: u32) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut indent = 0usize;
            for ch in l.chars() {
                match ch {
                    ' ' => indent += 1,
                    '\t' => indent += tab_size as usize - (indent % tab_size as usize),
                    _ => break,
                }
            }
            indent
        })
        .min()
        .unwrap_or(0);

    if min_indent == 0 {
        return text.to_string();
    }

    lines
        .iter()
        .map(|line| {
            let mut col = 0usize;
            let mut byte_start = 0;
            for (i, ch) in line.char_indices() {
                if col >= min_indent {
                    byte_start = i;
                    break;
                }
                match ch {
                    ' ' => col += 1,
                    '\t' => col += tab_size as usize - (col % tab_size as usize),
                    _ => {
                        byte_start = i;
                        break;
                    }
                }
                byte_start = i + ch.len_utf8();
            }
            &line[byte_start..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_line() {
        let r = render_line("hello", 4);
        assert_eq!(r.text, "hello");
        assert_eq!(r.display_width, 5);
    }

    #[test]
    fn render_tabs() {
        let r = render_line("a\tb", 4);
        assert_eq!(r.text, "a   b");
        assert_eq!(r.display_width, 5);
    }

    #[test]
    fn render_tab_alignment() {
        let r = render_line("\t", 4);
        assert_eq!(r.text, "    ");
        assert_eq!(r.display_width, 4);

        let r2 = render_line("ab\t", 4);
        assert_eq!(r2.text, "ab  ");
        assert_eq!(r2.display_width, 4);
    }

    #[test]
    fn display_width_simple() {
        assert_eq!(display_width("hello", 4), 5);
        assert_eq!(display_width("a\tb", 4), 5);
    }

    #[test]
    fn display_width_wide_chars() {
        // CJK character takes 2 columns
        assert_eq!(display_width("你好", 4), 4);
    }

    #[test]
    fn truncate() {
        assert_eq!(truncate_to_width("hello world", 5, 4), "hello");
        assert_eq!(truncate_to_width("hello world", 7, 4), "hello w");
    }

    #[test]
    fn whitespace_rendering_all() {
        assert_eq!(render_whitespace("a b", WhitespaceRender::All), "a·b");
    }

    #[test]
    fn whitespace_rendering_trailing() {
        assert_eq!(
            render_whitespace("hello  ", WhitespaceRender::Trailing),
            "hello··"
        );
        assert_eq!(
            render_whitespace("hello", WhitespaceRender::Trailing),
            "hello"
        );
    }

    #[test]
    fn whitespace_rendering_none() {
        assert_eq!(
            render_whitespace("a b  ", WhitespaceRender::None),
            "a b  "
        );
    }

    #[test]
    fn render_error_display() {
        let e = RenderError::InvalidTabSize(0);
        assert_eq!(e.to_string(), "invalid tab size: 0 (must be >= 1)");
        let e2 = RenderError::ColumnOutOfBounds {
            requested: 10,
            max: 5,
        };
        assert!(e2.to_string().contains("out of bounds"));
        assert_eq!(RenderError::ZeroWidth.to_string(), "width must be greater than zero");
    }

    #[test]
    fn rendered_line_display_trait() {
        let r = render_line("abc", 4);
        assert_eq!(format!("{}", r), "abc");
    }

    #[test]
    fn rendered_line_offset_at_column() {
        let r = render_line("a\tb", 4);
        assert_eq!(r.offset_at_column(0), Ok(0));
        // columns 1-3 map back to the tab
        assert_eq!(r.offset_at_column(1), Ok(1));
        assert_eq!(r.offset_at_column(4), Ok(2));
        assert!(r.offset_at_column(99).is_err());
    }

    #[test]
    fn rendered_line_is_empty() {
        let r = render_line("", 4);
        assert!(r.is_empty());
        let r2 = render_line("x", 4);
        assert!(!r2.is_empty());
    }

    #[test]
    fn byte_offset_to_column_basic() {
        assert_eq!(byte_offset_to_column("hello", 3, 4), 3);
        assert_eq!(byte_offset_to_column("a\tb", 2, 4), 4);
    }

    #[test]
    fn column_to_byte_offset_basic() {
        assert_eq!(column_to_byte_offset("hello", 3, 4), Some(3));
        assert_eq!(column_to_byte_offset("a\tb", 4, 4), Some(2));
        assert_eq!(column_to_byte_offset("hi", 99, 4), None);
    }

    #[test]
    fn pad_to_width_pads_short() {
        let s = pad_to_width("hi", 6, 4);
        assert_eq!(s, "hi    ");
        assert_eq!(display_width(&s, 4), 6);
    }

    #[test]
    fn pad_to_width_truncates_long() {
        let s = pad_to_width("hello world", 5, 4);
        assert_eq!(s, "hello");
    }

    #[test]
    fn render_config_builder_valid() {
        let cfg = RenderConfig::builder()
            .tab_size(2)
            .max_width(80)
            .whitespace_mode(WhitespaceRender::All)
            .build()
            .unwrap();
        assert_eq!(cfg.tab_size, 2);
        assert_eq!(cfg.max_width, 80);
        assert_eq!(cfg.whitespace_mode, WhitespaceRender::All);
    }

    #[test]
    fn render_config_builder_invalid_tab() {
        let err = RenderConfig::builder().tab_size(0).build();
        assert!(err.is_err());
        assert_eq!(err.unwrap_err(), RenderError::InvalidTabSize(0));
    }

    #[test]
    fn render_config_render_with_max_width() {
        let cfg = RenderConfig::builder()
            .tab_size(4)
            .max_width(5)
            .build()
            .unwrap();
        let r = cfg.render("hello world").unwrap();
        assert!(r.display_width <= 5);
    }

    #[test]
    fn render_control_character() {
        let r = render_line("\x01", 4);
        assert_eq!(r.text, "\u{2401}");
        assert_eq!(r.display_width, 1);
    }

    #[test]
    fn wrap_text_none_mode() {
        let lines = wrap_text("hello world", 5, TextWrapMode::None);
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn wrap_text_character_mode() {
        let lines = wrap_text("abcdefgh", 3, TextWrapMode::Character);
        assert_eq!(lines, vec!["abc", "def", "gh"]);
    }

    #[test]
    fn wrap_text_word_mode() {
        let lines = wrap_text("hello beautiful world", 14, TextWrapMode::Word);
        assert_eq!(lines, vec!["hello", "beautiful", "world"]);
    }

    #[test]
    fn wrap_text_wide_chars_character() {
        // Each CJK char is 2 columns wide, so max_width=4 fits 2 per line.
        let lines = wrap_text("你好世界", 4, TextWrapMode::Character);
        assert_eq!(lines, vec!["你好", "世界"]);
    }

    #[test]
    fn measure_text_ascii() {
        let m = measure_text("hello");
        assert_eq!(m.visible_width, 5);
        assert_eq!(m.byte_length, 5);
        assert_eq!(m.char_count, 5);
        assert!(!m.contains_wide_chars);
    }

    #[test]
    fn measure_text_wide() {
        let m = measure_text("你好");
        assert_eq!(m.visible_width, 4);
        assert_eq!(m.byte_length, 6);
        assert_eq!(m.char_count, 2);
        assert!(m.contains_wide_chars);
    }

    #[test]
    fn measure_text_mixed() {
        let m = measure_text("hi你");
        assert_eq!(m.visible_width, 4);
        assert_eq!(m.char_count, 3);
        assert!(m.contains_wide_chars);
    }

    #[test]
    fn truncate_with_ellipsis_no_truncation() {
        assert_eq!(truncate_with_ellipsis("hi", 10, "..."), "hi");
    }

    #[test]
    fn truncate_with_ellipsis_basic() {
        assert_eq!(truncate_with_ellipsis("hello world", 8, "..."), "hello...");
    }

    #[test]
    fn truncate_with_ellipsis_wide_chars() {
        // "你好世界" is 8 columns; truncate to 6 with "…" (1 col)
        let result = truncate_with_ellipsis("你好世界", 6, "…");
        assert_eq!(result, "你好…");
    }

    #[test]
    fn truncate_line_with_ellipsis_no_truncation() {
        assert_eq!(truncate_line_with_ellipsis("short", 10, 4, "..."), "short");
    }

    #[test]
    fn truncate_line_with_ellipsis_truncates() {
        let result = truncate_line_with_ellipsis("hello world foo", 10, 4, "...");
        assert!(result.ends_with("..."));
        assert!(display_width(&result, 4) <= 10);
    }

    #[test]
    fn detect_bidi_hint_ltr() {
        assert_eq!(detect_bidi_hint("hello"), BidiHint::LeftToRight);
    }

    #[test]
    fn detect_bidi_hint_rtl() {
        assert_eq!(detect_bidi_hint("\u{0627}\u{0644}"), BidiHint::RightToLeft);
    }

    #[test]
    fn detect_bidi_hint_neutral() {
        assert_eq!(detect_bidi_hint(""), BidiHint::Neutral);
        assert_eq!(detect_bidi_hint("   "), BidiHint::Neutral);
    }

    #[test]
    fn visualize_control_chars_replaces() {
        let input = "a\x01b\x02c";
        let output = visualize_control_chars(input);
        assert_eq!(output, "a\u{2401}b\u{2402}c");
    }

    #[test]
    fn visualize_control_chars_preserves_tab_and_newline() {
        let input = "a\tb\nc";
        let output = visualize_control_chars(input);
        assert_eq!(output, "a\tb\nc");
    }

    #[test]
    fn count_control_chars_works() {
        assert_eq!(count_control_chars("abc"), 0);
        assert_eq!(count_control_chars("a\x01\x02b"), 2);
        assert_eq!(count_control_chars("a\tb\n"), 0); // tab and LF excluded
    }

    #[test]
    fn pad_with_char_pads_short() {
        let s = pad_to_width_with_char("hi", 6, 4, '.');
        assert_eq!(s, "hi....");
    }

    #[test]
    fn pad_with_char_truncates_long() {
        let s = pad_to_width_with_char("hello world", 5, 4, '.');
        assert_eq!(s, "hello");
    }

    #[test]
    fn tab_expand_basic() {
        assert_eq!(tab_expand("a\tb", 4).unwrap(), "a   b");
        assert_eq!(tab_expand("\t", 4).unwrap(), "    ");
        assert_eq!(tab_expand("ab\tc", 4).unwrap(), "ab  c");
    }

    #[test]
    fn tab_expand_no_tabs() {
        assert_eq!(tab_expand("hello", 4).unwrap(), "hello");
    }

    #[test]
    fn tab_expand_invalid_tab_size() {
        assert!(tab_expand("x", 0).is_err());
    }

    #[test]
    fn tab_expand_custom_size() {
        assert_eq!(tab_expand("\t", 2).unwrap(), "  ");
        assert_eq!(tab_expand("a\t", 2).unwrap(), "a ");
    }

    #[test]
    fn center_text_basic() {
        let centered = center_text("hi", 10, 4);
        assert_eq!(centered.len(), 10);
        assert!(centered.starts_with("    "));
        assert!(centered.ends_with("    "));
    }

    #[test]
    fn center_text_wider_than_width() {
        assert_eq!(center_text("hello world", 5, 4), "hello world");
    }

    #[test]
    fn center_text_odd_padding() {
        let centered = center_text("x", 4, 4);
        assert_eq!(centered, " x  ");
    }

    #[test]
    fn right_align_basic() {
        let aligned = right_align("hi", 10, 4);
        assert_eq!(aligned, "        hi");
    }

    #[test]
    fn right_align_wider_than_width() {
        assert_eq!(right_align("hello world", 5, 4), "hello world");
    }

    #[test]
    fn visible_line_count_no_wrap() {
        assert_eq!(visible_line_count("hello world", 5, TextWrapMode::None), 1);
    }

    #[test]
    fn visible_line_count_character_wrap() {
        assert_eq!(visible_line_count("abcdefgh", 3, TextWrapMode::Character), 3);
    }

    #[test]
    fn column_align_basic() {
        let rows = vec![
            vec!["Name".into(), "Age".into()],
            vec!["Alice".into(), "30".into()],
            vec!["Bob".into(), "25".into()],
        ];
        let aligned = column_align(&rows, " | ", 4);
        assert_eq!(aligned.len(), 3);
        assert!(aligned[0].starts_with("Name "));
        assert!(aligned[1].starts_with("Alice"));
    }

    #[test]
    fn column_align_empty() {
        let aligned = column_align(&[], " ", 4);
        assert!(aligned.is_empty());
    }

    #[test]
    fn dedent_removes_common_indent() {
        let text = "    hello\n    world";
        let result = dedent(text, 4);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn dedent_preserves_relative_indent() {
        let text = "    hello\n        world";
        let result = dedent(text, 4);
        assert_eq!(result, "hello\n    world");
    }

    #[test]
    fn dedent_no_indent() {
        let text = "hello\nworld";
        assert_eq!(dedent(text, 4), "hello\nworld");
    }
}
