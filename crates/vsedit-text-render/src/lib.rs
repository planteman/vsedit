//! Unicode-aware terminal text rendering.
//!
//! Equivalent to VS Code's text rendering pipeline, adapted for terminal output.
//! Handles wide characters, tab expansion, and control characters.

use std::collections::HashMap;
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
    /// Return the number of characters in the rendered text.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Check whether the rendered text contains the given pattern.
    pub fn contains(&self, pattern: &str) -> bool {
        self.text.contains(pattern)
    }

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
    /// Create a default configuration with tab_size=4, no max width, no whitespace rendering.
    pub fn default_config() -> Self {
        Self::default()
    }

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

impl WhitespaceRender {
    /// Return a human-readable label for this whitespace render mode.
    pub fn label(&self) -> &'static str {
        match self {
            WhitespaceRender::None => "none",
            WhitespaceRender::Boundary => "boundary",
            WhitespaceRender::Selection => "selection",
            WhitespaceRender::Trailing => "trailing",
            WhitespaceRender::All => "all",
        }
    }
}

/// Count the number of leading whitespace characters in `text`.
pub fn count_leading_whitespace(text: &str) -> usize {
    text.chars().take_while(|c| c.is_whitespace()).count()
}

/// Count the number of trailing whitespace characters in `text`.
pub fn count_trailing_whitespace(text: &str) -> usize {
    text.chars().rev().take_while(|c| c.is_whitespace()).count()
}

/// Normalize line endings by converting `\r\n` sequences to `\n`.
pub fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Count the number of visible character columns in `text`, expanding tabs
/// to `tab_size` aligned stops and skipping line-ending characters.
pub fn visible_char_count(text: &str, tab_size: u32) -> usize {
    let mut count: usize = 0;
    for ch in text.chars() {
        match ch {
            '\t' => {
                let spaces = tab_size as usize - (count % tab_size as usize);
                count += spaces;
            }
            '\r' | '\n' => {}
            c => {
                count += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }
    }
    count
}

// ---------------------------------------------------------------------------
// TextLayoutEngine – lay out text within a bounding box with wrapping
// ---------------------------------------------------------------------------

/// A laid-out line within a bounding box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutLine {
    /// The text content of this visual line.
    pub text: String,
    /// Display width of this line.
    pub width: usize,
    /// Index of the source line this visual line originated from.
    pub source_line: usize,
    /// Whether this line is a continuation (wrapped) from the previous.
    pub is_wrapped: bool,
}

/// Engine that lays out text within a bounding box.
#[derive(Debug, Clone)]
pub struct TextLayoutEngine {
    /// Maximum display width for wrapping.
    pub max_width: usize,
    /// Tab size for expansion.
    pub tab_size: u32,
}

impl TextLayoutEngine {
    pub fn new(max_width: usize, tab_size: u32) -> Self {
        Self { max_width, tab_size }
    }

    /// Lay out the given text, wrapping lines that exceed `max_width`.
    pub fn layout(&self, text: &str) -> Vec<LayoutLine> {
        let mut result = Vec::new();
        for (line_idx, raw_line) in text.lines().enumerate() {
            let rendered = render_line(raw_line, self.tab_size);
            if self.max_width == 0 || rendered.display_width <= self.max_width {
                result.push(LayoutLine {
                    text: rendered.text,
                    width: rendered.display_width,
                    source_line: line_idx,
                    is_wrapped: false,
                });
            } else {
                let chars: Vec<char> = rendered.text.chars().collect();
                let mut pos = 0;
                let mut first = true;
                while pos < chars.len() {
                    let end = (pos + self.max_width).min(chars.len());
                    let segment: String = chars[pos..end].iter().collect();
                    let w = UnicodeWidthStr::width(segment.as_str());
                    result.push(LayoutLine {
                        text: segment,
                        width: w,
                        source_line: line_idx,
                        is_wrapped: !first,
                    });
                    pos = end;
                    first = false;
                }
            }
        }
        result
    }

    /// Total number of visual lines after layout.
    pub fn visual_line_count(&self, text: &str) -> usize {
        self.layout(text).len()
    }
}

// ---------------------------------------------------------------------------
// IndentationDetector – detect indent style from text
// ---------------------------------------------------------------------------

/// Detected indentation style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentStyle {
    Tabs,
    Spaces(u32),
    Mixed,
    Unknown,
}

/// Analyze text to detect the predominant indentation style.
pub struct IndentationDetector;

impl IndentationDetector {
    /// Detect the indentation style from a block of text.
    pub fn detect(text: &str) -> IndentStyle {
        let mut tab_lines = 0u32;
        let mut space_lines = 0u32;
        let mut space_widths: Vec<u32> = Vec::new();

        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let first = line.chars().next().unwrap_or(' ');
            if first == '\t' {
                tab_lines += 1;
            } else if first == ' ' {
                let count = line.chars().take_while(|c| *c == ' ').count() as u32;
                if count > 0 {
                    space_lines += 1;
                    space_widths.push(count);
                }
            }
        }

        if tab_lines == 0 && space_lines == 0 {
            return IndentStyle::Unknown;
        }
        if tab_lines > 0 && space_lines > 0 {
            return IndentStyle::Mixed;
        }
        if tab_lines > 0 {
            return IndentStyle::Tabs;
        }

        // Determine most common space width via GCD of indentation widths.
        let gcd = space_widths.iter().copied().fold(0u32, gcd_u32);
        if gcd == 0 { IndentStyle::Spaces(4) } else { IndentStyle::Spaces(gcd) }
    }
}

fn gcd_u32(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd_u32(b, a % b) }
}

// ---------------------------------------------------------------------------
// SyntaxHighlightRegion – mark regions for highlighting
// ---------------------------------------------------------------------------

/// A region in a text line that should receive syntax highlighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxHighlightRegion {
    /// 0-based start column (display column).
    pub start: usize,
    /// 0-based end column (exclusive).
    pub end: usize,
    /// Token kind, e.g. "keyword", "string", "comment".
    pub kind: String,
}

impl SyntaxHighlightRegion {
    pub fn new(start: usize, end: usize, kind: &str) -> Self {
        Self { start, end, kind: kind.to_string() }
    }

    /// Length of this region in display columns.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether two regions overlap.
    pub fn overlaps(&self, other: &SyntaxHighlightRegion) -> bool {
        self.start < other.end && other.start < self.end
    }
}

// ---------------------------------------------------------------------------
// text_to_grid – render text to a 2D char grid
// ---------------------------------------------------------------------------

/// Render text into a 2D grid of characters with the given width and height.
/// Cells beyond the text content are filled with spaces.
pub fn text_to_grid(text: &str, width: usize, height: usize, tab_size: u32) -> Vec<Vec<char>> {
    let mut grid = vec![vec![' '; width]; height];
    for (row, line) in text.lines().enumerate() {
        if row >= height {
            break;
        }
        let rendered = render_line(line, tab_size);
        for (col, ch) in rendered.text.chars().enumerate() {
            if col >= width {
                break;
            }
            grid[row][col] = ch;
        }
    }
    grid
}

/// Convert a grid back to a string, trimming trailing spaces on each row.
pub fn grid_to_string(grid: &[Vec<char>]) -> String {
    grid.iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip trailing whitespace from every line in a multi-line string.
pub fn strip_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Return the index (0-based) of the longest line in a multi-line string.
pub fn longest_line_index(text: &str, tab_size: u32) -> Option<usize> {
    text.lines()
        .enumerate()
        .max_by_key(|(_, line)| display_width(line, tab_size))
        .map(|(i, _)| i)
}

/// Compute the maximum display width across all lines.
pub fn max_line_width(text: &str, tab_size: u32) -> usize {
    text.lines()
        .map(|l| display_width(l, tab_size))
        .max()
        .unwrap_or(0)
}

/// Collapse consecutive blank lines into a single blank line.
pub fn collapse_blank_lines(text: &str) -> String {
    let mut result = Vec::new();
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

/// Indent every non-empty line by the given number of spaces.
pub fn indent_lines(text: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{}{}", prefix, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count the number of lines that exceed a given display width.
pub fn count_overlong_lines(text: &str, max_width: usize, tab_size: u32) -> usize {
    text.lines()
        .filter(|l| display_width(l, tab_size) > max_width)
        .count()
}

/// Extract lines from a multi-line string by a range of 0-based line indices.
pub fn extract_line_range(text: &str, start: usize, end: usize) -> String {
    text.lines()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// TextMeasurement – additional analysis methods
// ---------------------------------------------------------------------------

impl TextMeasurement {
    /// True when the text is pure ASCII with no wide characters.
    pub fn is_ascii_only(&self) -> bool {
        !self.contains_wide_chars && self.byte_length == self.char_count
    }

    /// Ratio of display width to character count.
    ///
    /// Returns 1.0 for pure narrow-character text, > 1.0 when wide characters
    /// are present, and 0.0 for empty text.
    pub fn width_ratio(&self) -> f64 {
        if self.char_count == 0 {
            return 0.0;
        }
        self.visible_width as f64 / self.char_count as f64
    }

    /// True when the measured text is empty.
    pub fn is_empty(&self) -> bool {
        self.byte_length == 0
    }
}

// ---------------------------------------------------------------------------
// RenderedLine – additional query and transformation methods
// ---------------------------------------------------------------------------

impl RenderedLine {
    /// Split the rendered text at the given display column, returning the
    /// left and right halves as new `String`s.
    ///
    /// If `col` is beyond the end of the text, the left half is the full text
    /// and the right half is empty.
    pub fn split_at_column(&self, col: usize) -> (String, String) {
        if col >= self.text.len() {
            return (self.text.clone(), String::new());
        }
        let mut current_col = 0usize;
        let mut byte_pos = 0usize;
        for (i, ch) in self.text.char_indices() {
            if current_col >= col {
                byte_pos = i;
                break;
            }
            let w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
            current_col += w;
            byte_pos = i + ch.len_utf8();
        }
        let left = self.text[..byte_pos].to_string();
        let right = self.text[byte_pos..].to_string();
        (left, right)
    }

    /// Return an iterator over display-column / source-offset pairs.
    pub fn column_offset_pairs(&self) -> Vec<(usize, usize)> {
        self.column_to_offset
            .iter()
            .enumerate()
            .map(|(col, &off)| (col, off))
            .collect()
    }

    /// Count the number of source byte offsets that map to this rendered line.
    ///
    /// This is the number of distinct source bytes referenced by the column map,
    /// which is useful for determining how many original characters contributed
    /// to the rendering.
    pub fn distinct_source_offsets(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for &off in &self.column_to_offset {
            seen.insert(off);
        }
        seen.len()
    }
}

// ---------------------------------------------------------------------------
// LayoutLine – query helpers
// ---------------------------------------------------------------------------

impl LayoutLine {
    /// True when this visual line is blank (whitespace only).
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// The display width remaining before `max_width` is reached.
    pub fn remaining_width(&self, max_width: usize) -> usize {
        max_width.saturating_sub(self.width)
    }
}

// ---------------------------------------------------------------------------
// TextLayoutEngine – additional layout helpers
// ---------------------------------------------------------------------------

impl TextLayoutEngine {
    /// Lay out text and return only lines whose source line index is in the
    /// given range `[start, end)`.
    pub fn layout_range(&self, text: &str, start: usize, end: usize) -> Vec<LayoutLine> {
        self.layout(text)
            .into_iter()
            .filter(|ll| ll.source_line >= start && ll.source_line < end)
            .collect()
    }

    /// Return the visual line index that corresponds to the given source line.
    ///
    /// Returns `None` if the source line does not exist.
    pub fn visual_line_for_source(&self, text: &str, source_line: usize) -> Option<usize> {
        self.layout(text)
            .iter()
            .position(|ll| ll.source_line == source_line)
    }

    /// Return the maximum display width across all visual lines produced by
    /// layout.
    pub fn max_visual_width(&self, text: &str) -> usize {
        self.layout(text)
            .iter()
            .map(|ll| ll.width)
            .max()
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// SyntaxHighlightRegion – merge and containment helpers
// ---------------------------------------------------------------------------

impl SyntaxHighlightRegion {
    /// True when `col` falls within this region (`start <= col < end`).
    pub fn contains_column(&self, col: usize) -> bool {
        col >= self.start && col < self.end
    }

    /// Merge two overlapping regions of the same kind into a single region.
    ///
    /// Returns `None` if the regions do not overlap or have different kinds.
    pub fn merge(&self, other: &SyntaxHighlightRegion) -> Option<SyntaxHighlightRegion> {
        if self.kind != other.kind || !self.overlaps(other) {
            return None;
        }
        Some(SyntaxHighlightRegion {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            kind: self.kind.clone(),
        })
    }

    /// Intersect two regions, returning the overlapping portion.
    ///
    /// Returns `None` if the regions do not overlap.
    pub fn intersect(&self, other: &SyntaxHighlightRegion) -> Option<SyntaxHighlightRegion> {
        if !self.overlaps(other) {
            return None;
        }
        Some(SyntaxHighlightRegion {
            start: self.start.max(other.start),
            end: self.end.min(other.end),
            kind: self.kind.clone(),
        })
    }

    /// Shift the region by `offset` columns to the right.
    pub fn shift(&self, offset: usize) -> SyntaxHighlightRegion {
        SyntaxHighlightRegion {
            start: self.start + offset,
            end: self.end + offset,
            kind: self.kind.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// IndentStyle – query helpers
// ---------------------------------------------------------------------------

impl IndentStyle {
    /// Return the effective number of spaces per indent level.
    ///
    /// Tabs default to `default_tab` spaces.  `Mixed` and `Unknown` also
    /// return `default_tab`.
    pub fn spaces_per_level(&self, default_tab: u32) -> u32 {
        match self {
            IndentStyle::Tabs => default_tab,
            IndentStyle::Spaces(n) => *n,
            IndentStyle::Mixed | IndentStyle::Unknown => default_tab,
        }
    }

    /// Human-readable label for display in status bars, etc.
    pub fn label(&self) -> &'static str {
        match self {
            IndentStyle::Tabs => "Tabs",
            IndentStyle::Spaces(n) if *n == 2 => "Spaces: 2",
            IndentStyle::Spaces(n) if *n == 4 => "Spaces: 4",
            IndentStyle::Spaces(_) => "Spaces",
            IndentStyle::Mixed => "Mixed",
            IndentStyle::Unknown => "Unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// RenderConfig – additional helpers
// ---------------------------------------------------------------------------

impl RenderConfig {
    /// Render multiple lines at once, returning results for each.
    pub fn render_lines(&self, text: &str) -> Result<Vec<RenderedLine>, RenderError> {
        self.validate()?;
        text.lines().map(|line| self.render(line)).collect()
    }

    /// Return a copy of this config with a different tab size.
    pub fn with_tab_size(&self, tab_size: u32) -> Self {
        Self {
            tab_size,
            max_width: self.max_width,
            whitespace_mode: self.whitespace_mode,
        }
    }

    /// Return a copy of this config with a different max width.
    pub fn with_max_width(&self, max_width: usize) -> Self {
        Self {
            tab_size: self.tab_size,
            max_width,
            whitespace_mode: self.whitespace_mode,
        }
    }
}

// ---------------------------------------------------------------------------
// TextRenderTabExpander
// ---------------------------------------------------------------------------

/// Expands tab characters to spaces with configurable tab-stop width.
#[derive(Debug, Clone)]
pub struct TextRenderTabExpander {
    tab_width: usize,
}

impl TextRenderTabExpander {
    /// Create a new tab expander with the given tab-stop width.
    ///
    /// # Panics
    /// Panics if `tab_width` is zero.
    pub fn new(tab_width: usize) -> Self {
        assert!(tab_width > 0, "tab_width must be at least 1");
        Self { tab_width }
    }

    /// Expand tabs in a single line, aligning to tab stops.
    pub fn expand(&self, line: &str) -> String {
        let mut result = String::with_capacity(line.len());
        let mut col: usize = 0;
        for ch in line.chars() {
            if ch == '\t' {
                let spaces = self.tab_width - (col % self.tab_width);
                for _ in 0..spaces {
                    result.push(' ');
                }
                col += spaces;
            } else {
                result.push(ch);
                col += UnicodeWidthChar::width(ch).unwrap_or(1);
            }
        }
        result
    }

    /// Expand tabs across all lines in a multi-line string.
    pub fn expand_all(&self, text: &str) -> String {
        text.lines()
            .map(|line| self.expand(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return the configured tab width.
    pub fn tab_width(&self) -> usize {
        self.tab_width
    }
}

impl fmt::Display for TextRenderTabExpander {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TabExpander(width={})", self.tab_width)
    }
}

// ---------------------------------------------------------------------------
// TextRenderTruncator
// ---------------------------------------------------------------------------

/// Truncates text to a maximum display width with configurable ellipsis placement.
#[derive(Debug, Clone)]
pub struct TextRenderTruncator {
    max_width: usize,
}

impl TextRenderTruncator {
    /// Create a new truncator with the given maximum display width.
    pub fn new(max_width: usize) -> Self {
        Self { max_width }
    }

    /// Return the configured maximum width.
    pub fn max_width(&self) -> usize {
        self.max_width
    }

    /// Returns `true` if the text's display width exceeds `max_width`.
    pub fn would_truncate(&self, text: &str) -> bool {
        UnicodeWidthStr::width(text) > self.max_width
    }

    /// Truncate from the end, appending "…" if needed.
    pub fn truncate_end(&self, text: &str) -> String {
        let text_width = UnicodeWidthStr::width(text);
        if text_width <= self.max_width {
            return text.to_string();
        }
        let ellipsis = "…";
        let ellipsis_width = UnicodeWidthStr::width(ellipsis);
        if self.max_width <= ellipsis_width {
            return ellipsis.to_string();
        }
        let target = self.max_width - ellipsis_width;
        let mut result = String::new();
        let mut col: usize = 0;
        for ch in text.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if col + cw > target {
                break;
            }
            result.push(ch);
            col += cw;
        }
        result.push_str(ellipsis);
        result
    }

    /// Truncate from the start, prepending "…" if needed.
    pub fn truncate_start(&self, text: &str) -> String {
        let text_width = UnicodeWidthStr::width(text);
        if text_width <= self.max_width {
            return text.to_string();
        }
        let ellipsis = "…";
        let ellipsis_width = UnicodeWidthStr::width(ellipsis);
        if self.max_width <= ellipsis_width {
            return ellipsis.to_string();
        }
        let target = self.max_width - ellipsis_width;
        // Walk backwards to collect `target` columns from the end.
        let chars: Vec<char> = text.chars().collect();
        let mut col: usize = 0;
        let mut start_idx = chars.len();
        for i in (0..chars.len()).rev() {
            let cw = UnicodeWidthChar::width(chars[i]).unwrap_or(0);
            if col + cw > target {
                break;
            }
            col += cw;
            start_idx = i;
        }
        let mut result = String::from(ellipsis);
        for &ch in &chars[start_idx..] {
            result.push(ch);
        }
        result
    }

    /// Truncate from the middle, inserting "…" in the center.
    pub fn truncate_middle(&self, text: &str) -> String {
        let text_width = UnicodeWidthStr::width(text);
        if text_width <= self.max_width {
            return text.to_string();
        }
        let ellipsis = "…";
        let ellipsis_width = UnicodeWidthStr::width(ellipsis);
        if self.max_width <= ellipsis_width {
            return ellipsis.to_string();
        }
        let available = self.max_width - ellipsis_width;
        let left_budget = (available + 1) / 2;
        let right_budget = available / 2;

        // Collect left portion.
        let mut left = String::new();
        let mut col: usize = 0;
        let chars: Vec<char> = text.chars().collect();
        let mut left_end = 0;
        for (i, &ch) in chars.iter().enumerate() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if col + cw > left_budget {
                break;
            }
            left.push(ch);
            col += cw;
            left_end = i + 1;
        }

        // Collect right portion from the end.
        let mut right_chars: Vec<char> = Vec::new();
        let mut rcol: usize = 0;
        for &ch in chars.iter().rev() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if rcol + cw > right_budget {
                break;
            }
            right_chars.push(ch);
            rcol += cw;
        }
        right_chars.reverse();
        let _ = left_end; // suppress unused warning

        let mut result = left;
        result.push_str(ellipsis);
        for ch in right_chars {
            result.push(ch);
        }
        result
    }
}

impl fmt::Display for TextRenderTruncator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Truncator(max_width={})", self.max_width)
    }
}

// ---------------------------------------------------------------------------
// TextRenderLineNumber
// ---------------------------------------------------------------------------

/// Generates line number gutter text with right-aligned numbers.
#[derive(Debug, Clone)]
pub struct TextRenderLineNumber {
    total_lines: usize,
    width: usize,
}

impl TextRenderLineNumber {
    /// Create a new line number formatter for a document with `total_lines` lines.
    pub fn new(total_lines: usize) -> Self {
        let width = if total_lines == 0 {
            1
        } else {
            total_lines.to_string().len()
        };
        Self { total_lines, width }
    }

    /// Format a line number, right-aligned to the gutter width.
    pub fn format_line_number(&self, line: usize) -> String {
        format!("{:>width$}", line, width = self.width)
    }

    /// Return the width of the gutter (number of digit columns).
    pub fn gutter_width(&self) -> usize {
        self.width
    }

    /// Return the separator string placed after the line number.
    pub fn separator(&self) -> &str {
        " | "
    }

    /// Format a line number with the separator appended, e.g. `"  42 | "`.
    pub fn format_with_separator(&self, line: usize) -> String {
        format!("{}{}", self.format_line_number(line), self.separator())
    }
}

impl fmt::Display for TextRenderLineNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LineNumber(total={}, gutter_width={})",
            self.total_lines, self.width
        )
    }
}

// ---------------------------------------------------------------------------
// TextRenderDiffHighlight
// ---------------------------------------------------------------------------

/// The kind of change a diff line represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Added,
    Deleted,
    Unchanged,
}

impl fmt::Display for DiffLineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffLineKind::Added => write!(f, "+"),
            DiffLineKind::Deleted => write!(f, "-"),
            DiffLineKind::Unchanged => write!(f, " "),
        }
    }
}

/// A single line in a rendered diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub line_num: usize,
    pub text: String,
    pub kind: DiffLineKind,
}

impl fmt::Display for DiffLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.text)
    }
}

/// Collects diff additions, deletions, and unchanged lines for rendering.
#[derive(Debug, Clone)]
pub struct TextRenderDiffHighlight {
    lines: Vec<DiffLine>,
}

impl TextRenderDiffHighlight {
    /// Create a new, empty diff highlighter.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Record an added line.
    pub fn add_addition(&mut self, line_num: usize, text: &str) {
        self.lines.push(DiffLine {
            line_num,
            text: text.to_string(),
            kind: DiffLineKind::Added,
        });
    }

    /// Record a deleted line.
    pub fn add_deletion(&mut self, line_num: usize, text: &str) {
        self.lines.push(DiffLine {
            line_num,
            text: text.to_string(),
            kind: DiffLineKind::Deleted,
        });
    }

    /// Record an unchanged context line.
    pub fn add_unchanged(&mut self, line_num: usize, text: &str) {
        self.lines.push(DiffLine {
            line_num,
            text: text.to_string(),
            kind: DiffLineKind::Unchanged,
        });
    }

    /// Return all collected diff lines.
    pub fn render(&self) -> Vec<DiffLine> {
        self.lines.clone()
    }

    /// Count the number of added lines.
    pub fn addition_count(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Added).count()
    }

    /// Count the number of deleted lines.
    pub fn deletion_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Deleted)
            .count()
    }
}

impl fmt::Display for TextRenderDiffHighlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// TextWrapCalculator – wrap text to width
// ---------------------------------------------------------------------------

/// Wraps text to a given display width, respecting grapheme clusters.
pub struct TextWrapCalculator;

impl TextWrapCalculator {
    /// Wrap a single line at the given column width.
    pub fn wrap_line(line: &str, width: usize) -> Vec<String> {
        if width == 0 { return vec![line.to_string()]; }
        let mut result = Vec::new();
        let mut current = String::new();
        let mut col = 0usize;
        for ch in line.chars() {
            let w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if col + w > width && !current.is_empty() {
                result.push(current.clone());
                current.clear();
                col = 0;
            }
            current.push(ch);
            col += w;
        }
        if !current.is_empty() {
            result.push(current);
        }
        if result.is_empty() {
            result.push(String::new());
        }
        result
    }

    /// Wrap multiple lines.
    pub fn wrapped_lines(lines: &[&str], width: usize) -> Vec<String> {
        lines.iter().flat_map(|l| Self::wrap_line(l, width)).collect()
    }

    /// Wrap at word boundaries when possible.
    pub fn wrap_at_word_boundary(line: &str, width: usize) -> Vec<String> {
        if width == 0 { return vec![line.to_string()]; }
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut result = Vec::new();
        let mut current = String::new();
        let mut col = 0usize;
        for word in words {
            let wlen = UnicodeWidthStr::width(word);
            if col > 0 && col + 1 + wlen > width {
                result.push(current.clone());
                current.clear();
                col = 0;
            }
            if col > 0 {
                current.push(' ');
                col += 1;
            }
            current.push_str(word);
            col += wlen;
        }
        if !current.is_empty() {
            result.push(current);
        }
        if result.is_empty() {
            result.push(String::new());
        }
        result
    }

    /// Return an overflow indicator if text exceeds width.
    pub fn overflow_indicator(line: &str, width: usize) -> Option<String> {
        let w = UnicodeWidthStr::width(line);
        if w > width {
            let mut truncated = String::new();
            let mut col = 0;
            for ch in line.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if col + cw + 1 > width { break; }
                truncated.push(ch);
                col += cw;
            }
            truncated.push('…');
            Some(truncated)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// TabExpander – expand tabs to spaces
// ---------------------------------------------------------------------------

/// Expands tab characters to spaces at configurable tab stops.
pub struct TabExpander;

impl TabExpander {
    /// Expand tabs in a string to spaces with the given tab size.
    pub fn expand_tabs(text: &str, tab_size: usize) -> String {
        let tab_size = tab_size.max(1);
        let mut result = String::new();
        let mut col = 0usize;
        for ch in text.chars() {
            if ch == '\t' {
                let spaces = tab_size - (col % tab_size);
                for _ in 0..spaces {
                    result.push(' ');
                }
                col += spaces;
            } else {
                result.push(ch);
                col += UnicodeWidthChar::width(ch).unwrap_or(1);
            }
        }
        result
    }

    /// Return tab stop column positions up to a given column.
    pub fn tab_stop_positions(tab_size: usize, up_to: usize) -> Vec<usize> {
        let tab_size = tab_size.max(1);
        (0..up_to).filter(|c| c % tab_size == 0).collect()
    }

    /// Return the column after expanding all tabs up to a position.
    pub fn column_after_expansion(text: &str, char_index: usize, tab_size: usize) -> usize {
        let tab_size = tab_size.max(1);
        let mut col = 0usize;
        for (i, ch) in text.chars().enumerate() {
            if i >= char_index { break; }
            if ch == '\t' {
                col += tab_size - (col % tab_size);
            } else {
                col += UnicodeWidthChar::width(ch).unwrap_or(1);
            }
        }
        col
    }

    /// Effective display width of the string after tab expansion.
    pub fn effective_width(text: &str, tab_size: usize) -> usize {
        Self::expand_tabs(text, tab_size).chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
    }
}

// ---------------------------------------------------------------------------
// LineBreakDetector – detect line break style
// ---------------------------------------------------------------------------

/// Detected line break style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBreakStyle {
    LF,
    CRLF,
    CR,
    Mixed,
}

/// Detects and converts line break styles.
pub struct LineBreakDetector;

impl LineBreakDetector {
    /// Detect the line break style used in the text.
    pub fn detect_from_text(text: &str) -> LineBreakStyle {
        let crlf_count = text.matches("\r\n").count();
        let text_no_crlf = text.replace("\r\n", "");
        let cr_count = text_no_crlf.matches('\r').count();
        let lf_count = text_no_crlf.matches('\n').count();

        let styles_present = [crlf_count > 0, cr_count > 0, lf_count > 0]
            .iter()
            .filter(|&&b| b)
            .count();

        if styles_present > 1 {
            LineBreakStyle::Mixed
        } else if crlf_count > 0 {
            LineBreakStyle::CRLF
        } else if cr_count > 0 {
            LineBreakStyle::CR
        } else {
            LineBreakStyle::LF
        }
    }

    /// Normalize all line breaks to a specific style.
    pub fn normalize_to(text: &str, style: LineBreakStyle) -> String {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        match style {
            LineBreakStyle::LF => normalized,
            LineBreakStyle::CRLF => normalized.replace('\n', "\r\n"),
            LineBreakStyle::CR => normalized.replace('\n', "\r"),
            LineBreakStyle::Mixed => normalized,
        }
    }

    /// Convert line breaks from one style to another.
    pub fn convert_line_breaks(text: &str, from: LineBreakStyle, to: LineBreakStyle) -> String {
        let _ = from;
        Self::normalize_to(text, to)
    }

    /// Count lines in the text.
    pub fn line_count(text: &str) -> usize {
        if text.is_empty() { return 0; }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        normalized.split('\n').count()
    }
}


/// Configuration manager for text_render functionality.
pub struct TextRenderConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl TextRenderConfig {
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

    pub fn merge(&mut self, other: &TextRenderConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for text_render operations.
pub struct TextRenderRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl TextRenderRateTracker {
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

/// Validation result collector for text_render.
pub struct TextRenderValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl TextRenderValidator {
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

    pub fn merge(&mut self, other: &TextRenderValidator) {
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
// xb_ utilities – batch 8
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer8 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer8 {
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
pub fn xb_fnv1a_8(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_8<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_8<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_8(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_8(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 181
// ---------------------------------------------------------------------------

/// Generic object pool `Xc181Pool<T>`.
pub struct Xc181Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc181Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc181PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc181Pool<T> {
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
    pub fn stats(&self) -> Xc181PoolStats {
        Xc181PoolStats {
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

impl<T> Default for Xc181Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc181Scheduler`.
pub struct Xc181Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc181Scheduler {
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

impl Default for Xc181Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_181 hash for the given byte slice.
pub fn xc_181_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_181 convention.
pub fn xc_181_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe20 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe20Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe20PipelineError {
    pub stage: Xe20Stage,
    pub message: String,
}

impl std::fmt::Display for Xe20PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe20Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe20Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError>>>,
    stage_names: Vec<Xe20Stage>,
}

impl Xe20Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe20Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe20Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe20Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe20Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
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

    pub fn compose(mut self, other: Xe20Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe20CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe20CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe20Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe20CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe20CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe20Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe20CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_20_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe20CacheEntry {
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

    fn xe_20_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe20CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_20_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
    Ok(data)
}

pub fn xe_20_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_20_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_20_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_20_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe20PipelineError> {
    Err(Xe20PipelineError {
        stage: Xe20Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #90
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf90Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf90TrieNode {
    children: std::collections::HashMap<char, Xf90TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf90Trie {
    root: Xf90TrieNode,
    count: usize,
}

impl Xf90Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf90TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf90TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf90TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf90BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf90BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 180).
pub struct Xh180SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh180SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 222 as u64,
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

/// A compact bit set supporting boolean operations (variant 180).
pub struct Xh180BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh180BitSet {
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

    #[test]
    fn rendered_line_char_count() {
        let r = render_line("hello", 4);
        assert_eq!(r.char_count(), 5);
        let r2 = render_line("a\tb", 4);
        // tab expands to 3 spaces: "a   b" = 5 chars
        assert_eq!(r2.char_count(), 5);
    }

    #[test]
    fn rendered_line_char_count_wide() {
        let r = render_line("你好", 4);
        assert_eq!(r.char_count(), 2);
    }

    #[test]
    fn rendered_line_contains() {
        let r = render_line("hello world", 4);
        assert!(r.contains("world"));
        assert!(!r.contains("xyz"));
    }

    #[test]
    fn render_config_default_config() {
        let cfg = RenderConfig::default_config();
        assert_eq!(cfg.tab_size, 4);
        assert_eq!(cfg.max_width, 0);
        assert_eq!(cfg.whitespace_mode, WhitespaceRender::None);
    }

    #[test]
    fn whitespace_render_label() {
        assert_eq!(WhitespaceRender::None.label(), "none");
        assert_eq!(WhitespaceRender::Boundary.label(), "boundary");
        assert_eq!(WhitespaceRender::Selection.label(), "selection");
        assert_eq!(WhitespaceRender::Trailing.label(), "trailing");
        assert_eq!(WhitespaceRender::All.label(), "all");
    }

    #[test]
    fn count_leading_whitespace_works() {
        assert_eq!(count_leading_whitespace("  hello"), 2);
        assert_eq!(count_leading_whitespace("hello"), 0);
        assert_eq!(count_leading_whitespace("\t hello"), 2);
        assert_eq!(count_leading_whitespace(""), 0);
    }

    #[test]
    fn count_trailing_whitespace_works() {
        assert_eq!(count_trailing_whitespace("hello  "), 2);
        assert_eq!(count_trailing_whitespace("hello"), 0);
        assert_eq!(count_trailing_whitespace("hello \t"), 2);
        assert_eq!(count_trailing_whitespace(""), 0);
    }

    #[test]
    fn normalize_line_endings_works() {
        assert_eq!(normalize_line_endings("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize_line_endings("a\nb"), "a\nb");
        assert_eq!(normalize_line_endings("no endings"), "no endings");
        assert_eq!(normalize_line_endings("\r\n\r\n"), "\n\n");
    }

    #[test]
    fn visible_char_count_works() {
        assert_eq!(visible_char_count("hello", 4), 5);
        assert_eq!(visible_char_count("a\tb", 4), 5);
        assert_eq!(visible_char_count("你好", 4), 4);
        assert_eq!(visible_char_count("\t", 4), 4);
        assert_eq!(visible_char_count("ab\r\n", 4), 2);
    }

    // ---- TextLayoutEngine tests ----

    #[test]
    fn layout_no_wrap() {
        let engine = TextLayoutEngine::new(80, 4);
        let lines = engine.layout("hello\nworld");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "hello");
        assert_eq!(lines[1].text, "world");
        assert!(!lines[0].is_wrapped);
    }

    #[test]
    fn layout_with_wrap() {
        let engine = TextLayoutEngine::new(5, 4);
        let lines = engine.layout("abcdefghij");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "abcde");
        assert!(lines[1].is_wrapped);
        assert_eq!(lines[1].text, "fghij");
    }

    #[test]
    fn layout_visual_line_count() {
        let engine = TextLayoutEngine::new(3, 4);
        assert_eq!(engine.visual_line_count("abcdef"), 2);
        assert_eq!(engine.visual_line_count("ab"), 1);
    }

    // ---- IndentationDetector tests ----

    #[test]
    fn detect_spaces() {
        let text = "  line1\n  line2\n    nested\n";
        assert_eq!(IndentationDetector::detect(text), IndentStyle::Spaces(2));
    }

    #[test]
    fn detect_tabs() {
        let text = "\tline1\n\tline2\n";
        assert_eq!(IndentationDetector::detect(text), IndentStyle::Tabs);
    }

    #[test]
    fn detect_mixed() {
        let text = "\tline1\n  line2\n";
        assert_eq!(IndentationDetector::detect(text), IndentStyle::Mixed);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(IndentationDetector::detect(""), IndentStyle::Unknown);
        assert_eq!(IndentationDetector::detect("no indent"), IndentStyle::Unknown);
    }

    // ---- SyntaxHighlightRegion tests ----

    #[test]
    fn highlight_region_overlap() {
        let a = SyntaxHighlightRegion::new(0, 5, "keyword");
        let b = SyntaxHighlightRegion::new(3, 8, "string");
        assert!(a.overlaps(&b));
        let c = SyntaxHighlightRegion::new(5, 8, "comment");
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn highlight_region_len() {
        let r = SyntaxHighlightRegion::new(2, 7, "keyword");
        assert_eq!(r.len(), 5);
        assert!(!r.is_empty());
        let empty = SyntaxHighlightRegion::new(3, 3, "x");
        assert!(empty.is_empty());
    }

    // ---- text_to_grid tests ----

    #[test]
    fn text_to_grid_basic() {
        let grid = text_to_grid("ab\ncd", 4, 3, 4);
        assert_eq!(grid.len(), 3);
        assert_eq!(grid[0][0], 'a');
        assert_eq!(grid[0][1], 'b');
        assert_eq!(grid[1][0], 'c');
        assert_eq!(grid[2][0], ' '); // empty row
    }

    #[test]
    fn grid_to_string_trims() {
        let grid = vec![
            vec!['a', 'b', ' ', ' '],
            vec!['c', ' ', ' ', ' '],
        ];
        assert_eq!(grid_to_string(&grid), "ab\nc");
    }

    #[test]
    fn strip_trailing_whitespace_removes_trailing() {
        assert_eq!(strip_trailing_whitespace("hello   \nworld  "), "hello\nworld");
        assert_eq!(strip_trailing_whitespace("no trailing"), "no trailing");
        assert_eq!(strip_trailing_whitespace(""), "");
    }

    #[test]
    fn longest_line_index_finds_longest() {
        assert_eq!(longest_line_index("short\na longer line\nmed", 4), Some(1));
        assert_eq!(longest_line_index("only", 4), Some(0));
    }

    #[test]
    fn max_line_width_computes_max() {
        assert_eq!(max_line_width("abc\nabcdef\nab", 4), 6);
        assert_eq!(max_line_width("", 4), 0);
        assert_eq!(max_line_width("\t", 4), 4);
    }

    #[test]
    fn collapse_blank_lines_collapses() {
        let input = "a\n\n\nb\n\nc";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "a\n\nb\n\nc");
    }

    #[test]
    fn collapse_blank_lines_no_blanks() {
        assert_eq!(collapse_blank_lines("a\nb\nc"), "a\nb\nc");
    }

    #[test]
    fn indent_lines_adds_spaces() {
        let result = indent_lines("hello\nworld", 4);
        assert_eq!(result, "    hello\n    world");
    }

    #[test]
    fn indent_lines_skips_blank() {
        let result = indent_lines("hello\n\nworld", 2);
        assert_eq!(result, "  hello\n\n  world");
    }

    #[test]
    fn count_overlong_lines_counts() {
        let text = "short\na very long line indeed\nok";
        assert_eq!(count_overlong_lines(text, 5, 4), 1);
        assert_eq!(count_overlong_lines(text, 100, 4), 0);
    }

    #[test]
    fn extract_line_range_slices() {
        let text = "zero\none\ntwo\nthree";
        assert_eq!(extract_line_range(text, 1, 3), "one\ntwo");
        assert_eq!(extract_line_range(text, 0, 1), "zero");
        assert_eq!(extract_line_range(text, 3, 3), "");
    }

    // ---- TextMeasurement impl tests ----

    #[test]
    fn text_measurement_is_ascii_only() {
        let m = measure_text("hello");
        assert!(m.is_ascii_only());
        let m2 = measure_text("你好");
        assert!(!m2.is_ascii_only());
    }

    #[test]
    fn text_measurement_width_ratio() {
        let m = measure_text("hello");
        assert!((m.width_ratio() - 1.0).abs() < f64::EPSILON);
        let m2 = measure_text("你好");
        assert!(m2.width_ratio() > 1.0);
        let m3 = measure_text("");
        assert!((m3.width_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn text_measurement_is_empty() {
        assert!(measure_text("").is_empty());
        assert!(!measure_text("x").is_empty());
    }

    // ---- RenderedLine split / query tests ----

    #[test]
    fn rendered_line_split_at_column() {
        let r = render_line("hello world", 4);
        let (left, right) = r.split_at_column(5);
        assert_eq!(left, "hello");
        assert_eq!(right, " world");
    }

    #[test]
    fn rendered_line_split_at_column_beyond_end() {
        let r = render_line("abc", 4);
        let (left, right) = r.split_at_column(100);
        assert_eq!(left, "abc");
        assert_eq!(right, "");
    }

    #[test]
    fn rendered_line_column_offset_pairs() {
        let r = render_line("ab", 4);
        let pairs = r.column_offset_pairs();
        assert_eq!(pairs, vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn rendered_line_distinct_source_offsets() {
        let r = render_line("a\tb", 4);
        // "a   b" — columns 0→offset 0, 1-3→offset 1 (tab), 4→offset 2
        // distinct offsets: {0, 1, 2} = 3
        assert_eq!(r.distinct_source_offsets(), 3);
    }

    // ---- LayoutLine tests ----

    #[test]
    fn layout_line_is_blank() {
        let ll = LayoutLine {
            text: "   ".to_string(),
            width: 3,
            source_line: 0,
            is_wrapped: false,
        };
        assert!(ll.is_blank());

        let ll2 = LayoutLine {
            text: "abc".to_string(),
            width: 3,
            source_line: 0,
            is_wrapped: false,
        };
        assert!(!ll2.is_blank());
    }

    #[test]
    fn layout_line_remaining_width() {
        let ll = LayoutLine {
            text: "abc".to_string(),
            width: 3,
            source_line: 0,
            is_wrapped: false,
        };
        assert_eq!(ll.remaining_width(10), 7);
        assert_eq!(ll.remaining_width(2), 0);
    }

    // ---- TextLayoutEngine extended tests ----

    #[test]
    fn layout_engine_layout_range() {
        let engine = TextLayoutEngine::new(80, 4);
        let text = "line0\nline1\nline2\nline3";
        let lines = engine.layout_range(text, 1, 3);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line1");
        assert_eq!(lines[1].text, "line2");
    }

    #[test]
    fn layout_engine_visual_line_for_source() {
        let engine = TextLayoutEngine::new(80, 4);
        let text = "aaa\nbbb\nccc";
        assert_eq!(engine.visual_line_for_source(text, 0), Some(0));
        assert_eq!(engine.visual_line_for_source(text, 2), Some(2));
        assert_eq!(engine.visual_line_for_source(text, 5), None);
    }

    #[test]
    fn layout_engine_max_visual_width() {
        let engine = TextLayoutEngine::new(80, 4);
        let text = "short\na much longer line\nmed";
        let max_w = engine.max_visual_width(text);
        assert_eq!(max_w, display_width("a much longer line", 4));
    }

    // ---- SyntaxHighlightRegion extended tests ----

    #[test]
    fn highlight_region_contains_column() {
        let r = SyntaxHighlightRegion::new(2, 7, "keyword");
        assert!(!r.contains_column(1));
        assert!(r.contains_column(2));
        assert!(r.contains_column(6));
        assert!(!r.contains_column(7));
    }

    #[test]
    fn highlight_region_merge() {
        let a = SyntaxHighlightRegion::new(0, 5, "keyword");
        let b = SyntaxHighlightRegion::new(3, 8, "keyword");
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 8);
        assert_eq!(merged.kind, "keyword");
    }

    #[test]
    fn highlight_region_merge_different_kind() {
        let a = SyntaxHighlightRegion::new(0, 5, "keyword");
        let b = SyntaxHighlightRegion::new(3, 8, "string");
        assert!(a.merge(&b).is_none());
    }

    #[test]
    fn highlight_region_intersect() {
        let a = SyntaxHighlightRegion::new(0, 5, "keyword");
        let b = SyntaxHighlightRegion::new(3, 8, "keyword");
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 3);
        assert_eq!(inter.end, 5);
    }

    #[test]
    fn highlight_region_intersect_no_overlap() {
        let a = SyntaxHighlightRegion::new(0, 3, "keyword");
        let b = SyntaxHighlightRegion::new(5, 8, "keyword");
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn highlight_region_shift() {
        let r = SyntaxHighlightRegion::new(2, 5, "comment");
        let shifted = r.shift(10);
        assert_eq!(shifted.start, 12);
        assert_eq!(shifted.end, 15);
        assert_eq!(shifted.kind, "comment");
    }

    // ---- IndentStyle tests ----

    #[test]
    fn indent_style_spaces_per_level() {
        assert_eq!(IndentStyle::Tabs.spaces_per_level(4), 4);
        assert_eq!(IndentStyle::Spaces(2).spaces_per_level(4), 2);
        assert_eq!(IndentStyle::Mixed.spaces_per_level(8), 8);
        assert_eq!(IndentStyle::Unknown.spaces_per_level(4), 4);
    }

    #[test]
    fn indent_style_label() {
        assert_eq!(IndentStyle::Tabs.label(), "Tabs");
        assert_eq!(IndentStyle::Spaces(2).label(), "Spaces: 2");
        assert_eq!(IndentStyle::Spaces(4).label(), "Spaces: 4");
        assert_eq!(IndentStyle::Spaces(3).label(), "Spaces");
        assert_eq!(IndentStyle::Mixed.label(), "Mixed");
        assert_eq!(IndentStyle::Unknown.label(), "Unknown");
    }

    // ---- RenderConfig extended tests ----

    #[test]
    fn render_config_render_lines() {
        let cfg = RenderConfig::default_config();
        let lines = cfg.render_lines("hello\nworld").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "hello");
        assert_eq!(lines[1].text, "world");
    }

    #[test]
    fn render_config_with_tab_size() {
        let cfg = RenderConfig::default_config();
        let cfg2 = cfg.with_tab_size(8);
        assert_eq!(cfg2.tab_size, 8);
        assert_eq!(cfg2.max_width, cfg.max_width);
    }

    #[test]
    fn render_config_with_max_width() {
        let cfg = RenderConfig::default_config();
        let cfg2 = cfg.with_max_width(120);
        assert_eq!(cfg2.max_width, 120);
        assert_eq!(cfg2.tab_size, cfg.tab_size);
    }

    // ---- TextRenderTabExpander tests ----

    #[test]
    fn tab_expander_basic() {
        let exp = TextRenderTabExpander::new(4);
        assert_eq!(exp.expand("a\tb"), "a   b");
        assert_eq!(exp.expand("\t"), "    ");
        assert_eq!(exp.expand("ab\tc"), "ab  c");
    }

    #[test]
    fn tab_expander_no_tabs() {
        let exp = TextRenderTabExpander::new(4);
        assert_eq!(exp.expand("hello"), "hello");
    }

    #[test]
    fn tab_expander_expand_all() {
        let exp = TextRenderTabExpander::new(4);
        let result = exp.expand_all("a\tb\ncd\te");
        assert_eq!(result, "a   b\ncd  e");
    }

    #[test]
    fn tab_expander_display_and_clone() {
        let exp = TextRenderTabExpander::new(8);
        assert_eq!(exp.tab_width(), 8);
        let display = format!("{}", exp);
        assert!(display.contains("8"));
        let cloned = exp.clone();
        assert_eq!(cloned.tab_width(), 8);
    }

    // ---- TextRenderTruncator tests ----

    #[test]
    fn truncator_end() {
        let tr = TextRenderTruncator::new(8);
        assert_eq!(tr.truncate_end("hello world!"), "hello w…");
        assert_eq!(tr.truncate_end("short"), "short");
    }

    #[test]
    fn truncator_start() {
        let tr = TextRenderTruncator::new(8);
        let result = tr.truncate_start("hello world!");
        assert!(result.starts_with('…'));
        assert_eq!(UnicodeWidthStr::width(result.as_str()), 8);
    }

    #[test]
    fn truncator_middle() {
        let tr = TextRenderTruncator::new(9);
        let result = tr.truncate_middle("hello world!");
        assert!(result.contains('…'));
        assert!(UnicodeWidthStr::width(result.as_str()) <= 9);
    }

    #[test]
    fn truncator_would_truncate() {
        let tr = TextRenderTruncator::new(5);
        assert!(tr.would_truncate("hello world"));
        assert!(!tr.would_truncate("hi"));
    }

    #[test]
    fn truncator_display() {
        let tr = TextRenderTruncator::new(42);
        assert_eq!(format!("{}", tr), "Truncator(max_width=42)");
        assert_eq!(tr.max_width(), 42);
    }

    // ---- TextRenderLineNumber tests ----

    #[test]
    fn line_number_formatting() {
        let ln = TextRenderLineNumber::new(100);
        assert_eq!(ln.gutter_width(), 3);
        assert_eq!(ln.format_line_number(1), "  1");
        assert_eq!(ln.format_line_number(42), " 42");
        assert_eq!(ln.format_line_number(100), "100");
    }

    #[test]
    fn line_number_with_separator() {
        let ln = TextRenderLineNumber::new(999);
        assert_eq!(ln.format_with_separator(42), " 42 | ");
        assert_eq!(ln.separator(), " | ");
        let display = format!("{}", ln);
        assert!(display.contains("999"));
    }

    // ---- TextRenderDiffHighlight tests ----

    #[test]
    fn diff_highlight_counts() {
        let mut diff = TextRenderDiffHighlight::new();
        diff.add_addition(1, "new line");
        diff.add_deletion(2, "old line");
        diff.add_unchanged(3, "context");
        diff.add_addition(4, "another new");
        assert_eq!(diff.addition_count(), 2);
        assert_eq!(diff.deletion_count(), 1);
        let rendered = diff.render();
        assert_eq!(rendered.len(), 4);
        assert_eq!(rendered[0].kind, DiffLineKind::Added);
        assert_eq!(rendered[1].kind, DiffLineKind::Deleted);
        assert_eq!(rendered[2].kind, DiffLineKind::Unchanged);
    }

    #[test]
    fn diff_line_display() {
        let line = DiffLine {
            line_num: 10,
            text: "hello".to_string(),
            kind: DiffLineKind::Added,
        };
        assert_eq!(format!("{}", line), "+ hello");
        assert_eq!(format!("{}", DiffLineKind::Deleted), "-");
        assert_eq!(format!("{}", DiffLineKind::Unchanged), " ");
    }

    #[test]
    fn diff_highlight_display() {
        let mut diff = TextRenderDiffHighlight::new();
        diff.add_addition(1, "added");
        diff.add_deletion(2, "removed");
        let output = format!("{}", diff);
        assert!(output.contains("+ added"));
        assert!(output.contains("- removed"));
    }

    // -- TextWrapCalculator -------------------------------------------------

    #[test]
    fn wrap_line_basic() {
        let wrapped = TextWrapCalculator::wrap_line("hello world", 5);
        assert!(wrapped.len() >= 1);
    }

    #[test]
    fn wrap_line_no_wrap_needed() {
        let wrapped = TextWrapCalculator::wrap_line("hi", 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "hi");
    }

    #[test]
    fn wrap_at_word_boundary() {
        let wrapped = TextWrapCalculator::wrap_at_word_boundary("hello beautiful world", 14);
        assert!(wrapped.len() >= 1);
    }

    #[test]
    fn wrap_overflow_indicator() {
        let ind = TextWrapCalculator::overflow_indicator("long text here", 8);
        assert!(ind.is_some());
        assert!(ind.unwrap().ends_with('…'));
    }

    #[test]
    fn wrap_no_overflow() {
        let ind = TextWrapCalculator::overflow_indicator("short", 10);
        assert!(ind.is_none());
    }

    // -- TabExpander --------------------------------------------------------

    #[test]
    fn tab_expand_basic_v2() {
        let expanded = TabExpander::expand_tabs("a\tb", 4);
        assert_eq!(expanded, "a   b");
    }

    #[test]
    fn tab_expand_multiple() {
        let expanded = TabExpander::expand_tabs("\t\t", 4);
        assert_eq!(expanded, "        ");
    }

    #[test]
    fn tab_stop_positions() {
        let stops = TabExpander::tab_stop_positions(4, 16);
        assert_eq!(stops, vec![0, 4, 8, 12]);
    }

    #[test]
    fn tab_effective_width() {
        let w = TabExpander::effective_width("a\tb", 4);
        assert_eq!(w, 5);
    }

    // -- LineBreakDetector --------------------------------------------------

    #[test]
    fn detect_lf() {
        assert_eq!(LineBreakDetector::detect_from_text("a\nb\nc"), LineBreakStyle::LF);
    }

    #[test]
    fn detect_crlf() {
        assert_eq!(LineBreakDetector::detect_from_text("a\r\nb\r\nc"), LineBreakStyle::CRLF);
    }

    #[test]
    fn detect_mixed_linebreak() {
        assert_eq!(LineBreakDetector::detect_from_text("a\nb\r\nc"), LineBreakStyle::Mixed);
    }

    #[test]
    fn normalize_to_crlf() {
        let result = LineBreakDetector::normalize_to("a\nb\nc", LineBreakStyle::CRLF);
        assert_eq!(result, "a\r\nb\r\nc");
    }

    #[test]
    fn line_count_basic() {
        assert_eq!(LineBreakDetector::line_count("a\nb\nc"), 3);
        assert_eq!(LineBreakDetector::line_count("a\r\nb"), 2);
        assert_eq!(LineBreakDetector::line_count(""), 0);
    }

    #[test]
    fn text_render_config_new() {
        let cfg = TextRenderConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn text_render_config_set_get() {
        let mut cfg = TextRenderConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn text_render_config_remove() {
        let mut cfg = TextRenderConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn text_render_config_keys_sorted() {
        let mut cfg = TextRenderConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn text_render_config_bump_version() {
        let mut cfg = TextRenderConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn text_render_config_clear() {
        let mut cfg = TextRenderConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn text_render_config_merge() {
        let mut cfg1 = TextRenderConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = TextRenderConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn text_render_config_disable() {
        let mut cfg = TextRenderConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn text_render_rate_tracker_empty() {
        let rt = TextRenderRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn text_render_rate_tracker_record() {
        let mut rt = TextRenderRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn text_render_rate_tracker_prune() {
        let mut rt = TextRenderRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn text_render_validator_valid() {
        let v = TextRenderValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn text_render_validator_errors() {
        let mut v = TextRenderValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn text_render_validator_clear() {
        let mut v = TextRenderValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn text_render_validator_merge() {
        let mut v1 = TextRenderValidator::new();
        v1.add_error("e1");
        let mut v2 = TextRenderValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn text_render_rate_tracker_clear() {
        let mut rt = TextRenderRateTracker::new(1000);
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
    fn xb_ring_buffer_8_push_and_len() {
        let mut rb = super::XbRingBuffer8::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_8_overwrite() {
        let mut rb = super::XbRingBuffer8::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_8_get_out_of_bounds() {
        let rb = super::XbRingBuffer8::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_8_drain_all() {
        let mut rb = super::XbRingBuffer8::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_8_peek_front_back() {
        let mut rb = super::XbRingBuffer8::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_8_clear() {
        let mut rb = super::XbRingBuffer8::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_8_capacity() {
        let rb = super::XbRingBuffer8::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_8_basic() {
        let h = super::xb_fnv1a_8(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_8(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_8_different_inputs() {
        let h1 = super::xb_fnv1a_8(b"abc");
        let h2 = super::xb_fnv1a_8(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_8_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_8(&data);
        let dec = super::xb_rle_decode_8(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_8_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_8(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_8(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_8_values() {
        assert!((super::xb_clamp_8(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_8(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_8(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_8_values() {
        assert!((super::xb_lerp_8(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_8(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_8(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_8_wrap_around_twice() {
        let mut rb = super::XbRingBuffer8::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 181 ----

    #[test]
    fn xc_181_pool_new_empty() {
        let pool: super::Xc181Pool<i32> = super::Xc181Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_181_pool_release_acquire() {
        let mut pool = super::Xc181Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_181_pool_acquire_empty() {
        let mut pool: super::Xc181Pool<i32> = super::Xc181Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_181_pool_full() {
        let mut pool = super::Xc181Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_181_pool_drain() {
        let mut pool = super::Xc181Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_181_pool_stats() {
        let mut pool = super::Xc181Pool::new(8);
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
    fn xc_181_pool_clear() {
        let mut pool = super::Xc181Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_181_pool_shrink() {
        let mut pool = super::Xc181Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_181_pool_default() {
        let pool: super::Xc181Pool<String> = super::Xc181Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_181_pool_extend() {
        let mut pool = super::Xc181Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_181_pool_retain() {
        let mut pool = super::Xc181Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_181_scheduler_round_robin() {
        let mut sched = super::Xc181Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_181_scheduler_empty() {
        let mut sched = super::Xc181Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_181_scheduler_reset() {
        let mut sched = super::Xc181Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_181_scheduler_add_remove() {
        let mut sched = super::Xc181Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_181_scheduler_targets() {
        let sched = super::Xc181Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_181_hash_empty() {
        assert_eq!(super::xc_181_hash(b""), 5381);
    }

    #[test]
    fn xc_181_hash_data() {
        let h = super::xc_181_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_181_hash(b"hello"), h);
    }

    #[test]
    fn xc_181_reverse_str() {
        assert_eq!(super::xc_181_reverse("abc"), "cba");
        assert_eq!(super::xc_181_reverse(""), "");
    }


    #[test]
    fn xe_20_pipeline_empty() {
        let p = super::Xe20Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_20_pipeline_parse_stage() {
        let p = super::Xe20Pipeline::new()
            .add_parse(super::xe_20_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_20_pipeline_transform_double() {
        let p = super::Xe20Pipeline::new()
            .add_transform(super::xe_20_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_20_pipeline_validate_reverse() {
        let p = super::Xe20Pipeline::new()
            .add_validate(super::xe_20_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_20_pipeline_emit_filter() {
        let p = super::Xe20Pipeline::new()
            .add_emit(super::xe_20_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_20_pipeline_multi_stage() {
        let p = super::Xe20Pipeline::new()
            .add_parse(super::xe_20_pipeline_identity)
            .add_transform(super::xe_20_pipeline_double)
            .add_validate(super::xe_20_pipeline_reverse)
            .add_emit(super::xe_20_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_20_pipeline_error_propagation() {
        let p = super::Xe20Pipeline::new()
            .add_parse(super::xe_20_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe20Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_20_pipeline_compose() {
        let p1 = super::Xe20Pipeline::new()
            .add_parse(super::xe_20_pipeline_identity);
        let p2 = super::Xe20Pipeline::new()
            .add_transform(super::xe_20_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_20_pipeline_error_display() {
        let e = super::Xe20PipelineError {
            stage: super::Xe20Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_20_cache_put_get() {
        let mut c = super::Xe20Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_20_cache_miss() {
        let mut c: super::Xe20Cache<&str, i32> = super::Xe20Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_20_cache_ttl_expiry() {
        let mut c = super::Xe20Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_20_cache_evict() {
        let mut c = super::Xe20Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_20_cache_capacity() {
        let mut c = super::Xe20Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_20_cache_stats() {
        let mut c = super::Xe20Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_20_cache_clear() {
        let mut c = super::Xe20Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #90 --

    #[test]
    fn xf90_trie_insert_search() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf90_trie_starts_with() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf90_trie_remove() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf90_trie_word_count() {
        let mut t = Xf90Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf90_trie_longest_prefix() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf90_trie_all_words() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf90_trie_autocomplete() {
        let mut t = Xf90Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf90_trie_empty_search() {
        let t = Xf90Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf90_bloom_add_contains() {
        let mut bf = Xf90BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf90_bloom_probably_absent() {
        let bf = Xf90BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf90_bloom_false_positive_rate() {
        let mut bf = Xf90BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf90_bloom_clear() {
        let mut bf = Xf90BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf90_bloom_union() {
        let mut a = Xf90BloomFilter::xf_new(512, 2);
        let mut b = Xf90BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf90_bloom_intersection_estimate() {
        let mut a = Xf90BloomFilter::xf_new(512, 2);
        let mut b = Xf90BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf90_bloom_union_size_mismatch() {
        let a = Xf90BloomFilter::xf_new(256, 2);
        let b = Xf90BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh180_skip_insert_contains() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh180_skip_remove() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh180_skip_len() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh180_skip_range_query() {
        let mut sl = super::Xh180SkipList::xh_new(4);
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
    fn xh180_skip_floor_ceiling() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh180_skip_rank() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh180_skip_empty() {
        let sl = super::Xh180SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh180_skip_duplicates() {
        let mut sl = super::Xh180SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh180_bitset_set_test() {
        let mut bs = super::Xh180BitSet::xh_new(256);
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
    fn xh180_bitset_clear_count() {
        let mut bs = super::Xh180BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh180_bitset_and_or_xor() {
        let mut a = super::Xh180BitSet::xh_new(128);
        let mut b = super::Xh180BitSet::xh_new(128);
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
    fn xh180_bitset_iter_ones() {
        let mut bs = super::Xh180BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh180_bitset_first_last() {
        let mut bs = super::Xh180BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh180_bitset_empty() {
        let bs = super::Xh180BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
