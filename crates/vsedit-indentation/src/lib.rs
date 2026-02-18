//! Indentation detection and manipulation.

use std::collections::HashMap;
use std::fmt;
/// Detected indentation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(u32),
    Tabs,
}

/// Detect the indentation style used in text.
pub fn detect_indentation(text: &str) -> IndentStyle {
    let mut space_counts: [u32; 9] = [0; 9]; // index 1-8
    let mut tab_count: u32 = 0;

    for line in text.lines() {
        if line.is_empty() { continue; }
        if line.starts_with('\t') {
            tab_count += 1;
        } else {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            if spaces > 0 && spaces <= 8 {
                space_counts[spaces] += 1;
            }
        }
    }

    if tab_count > space_counts.iter().sum::<u32>() / 2 {
        return IndentStyle::Tabs;
    }

    // Find most common space indent
    let mut best_size = 4u32;
    let mut best_count = 0u32;
    for size in [2u32, 4, 8, 3, 6] {
        let count = space_counts.iter().enumerate()
            .filter(|(i, _)| *i > 0 && *i % size as usize == 0)
            .map(|(_, c)| c)
            .sum::<u32>();
        if count > best_count {
            best_count = count;
            best_size = size;
        }
    }

    IndentStyle::Spaces(best_size)
}

/// Convert indentation between styles.
pub fn convert_indentation(text: &str, from: IndentStyle, to: IndentStyle) -> String {
    text.lines().map(|line| {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        let indent_count = match from {
            IndentStyle::Tabs => indent.matches('\t').count() as u32,
            IndentStyle::Spaces(n) => {
                let spaces = indent.len() as u32;
                if n > 0 { spaces / n } else { 0 }
            }
        };

        let new_indent = match to {
            IndentStyle::Tabs => "\t".repeat(indent_count as usize),
            IndentStyle::Spaces(n) => " ".repeat((indent_count * n) as usize),
        };

        format!("{}{}", new_indent, trimmed)
    }).collect::<Vec<_>>().join("\n")
}

impl std::fmt::Display for IndentStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndentStyle::Spaces(n) => write!(f, "Spaces({})", n),
            IndentStyle::Tabs => write!(f, "Tabs"),
        }
    }
}

impl IndentStyle {
    /// Return the single-level indent string for this style.
    pub fn indent_string(&self) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(*n as usize),
            IndentStyle::Tabs => "\t".to_string(),
        }
    }

    /// Return the indent string repeated `levels` times.
    pub fn indent_string_n(&self, levels: u32) -> String {
        self.indent_string().repeat(levels as usize)
    }
}

/// Count how many indent levels the line starts with for the given style.
pub fn get_line_indent_level(line: &str, style: IndentStyle) -> u32 {
    match style {
        IndentStyle::Tabs => {
            line.bytes().take_while(|&b| b == b'\t').count() as u32
        }
        IndentStyle::Spaces(n) => {
            if n == 0 {
                return 0;
            }
            let spaces = line.bytes().take_while(|&b| b == b' ').count() as u32;
            spaces / n
        }
    }
}

/// Indent every line in `text` by `levels` additional levels.
pub fn indent_lines(text: &str, style: IndentStyle, levels: u32) -> String {
    let prefix = style.indent_string_n(levels);
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", prefix, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove up to `levels` indent levels from every line in `text`.
pub fn dedent_lines(text: &str, style: IndentStyle, levels: u32) -> String {
    text.lines()
        .map(|line| {
            let current = get_line_indent_level(line, style);
            let remove = current.min(levels);
            let strip_len = match style {
                IndentStyle::Tabs => remove as usize,
                IndentStyle::Spaces(n) => (remove * n) as usize,
            };
            &line[strip_len..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert all indentation in `text` to the `target` style, auto-detecting the source style.
pub fn normalize_indentation(text: &str, target: IndentStyle) -> String {
    let detected = detect_indentation(text);
    if detected == target {
        return text.to_string();
    }
    convert_indentation(text, detected, target)
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during indentation operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentError {
    /// The requested space width is zero or exceeds the maximum (8).
    InvalidSpaceWidth(u32),
    /// The indent level would overflow the maximum allowed depth.
    MaxDepthExceeded { depth: u32, max: u32 },
    /// The input text contains mixed indentation that cannot be resolved.
    MixedIndentation,
}

impl std::fmt::Display for IndentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndentError::InvalidSpaceWidth(w) => {
                write!(f, "invalid space width {}: must be 1..=8", w)
            }
            IndentError::MaxDepthExceeded { depth, max } => {
                write!(f, "indent depth {} exceeds maximum {}", depth, max)
            }
            IndentError::MixedIndentation => {
                write!(f, "text contains mixed tabs and spaces")
            }
        }
    }
}

impl std::error::Error for IndentError {}

// ---------------------------------------------------------------------------
// IndentStyle — additional helpers
// ---------------------------------------------------------------------------

impl IndentStyle {
    /// Create a `Spaces` variant, validating width is 1..=8.
    pub fn spaces(width: u32) -> Result<Self, IndentError> {
        if width == 0 || width > 8 {
            return Err(IndentError::InvalidSpaceWidth(width));
        }
        Ok(IndentStyle::Spaces(width))
    }

    /// Return the visual width of a single indent level.
    pub fn visual_width(&self) -> u32 {
        match self {
            IndentStyle::Spaces(n) => *n,
            IndentStyle::Tabs => 4, // conventional tab display width
        }
    }

    /// Return `true` if this style uses spaces.
    pub fn is_spaces(&self) -> bool {
        matches!(self, IndentStyle::Spaces(_))
    }

    /// Return `true` if this style uses tabs.
    pub fn is_tabs(&self) -> bool {
        matches!(self, IndentStyle::Tabs)
    }
}

// ---------------------------------------------------------------------------
// IndentConfig — builder pattern
// ---------------------------------------------------------------------------

/// Configuration for indentation operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentConfig {
    pub style: IndentStyle,
    pub max_depth: u32,
    pub trim_trailing_whitespace: bool,
    pub final_newline: bool,
}

impl Default for IndentConfig {
    fn default() -> Self {
        Self {
            style: IndentStyle::Spaces(4),
            max_depth: 20,
            trim_trailing_whitespace: true,
            final_newline: true,
        }
    }
}

impl std::fmt::Display for IndentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IndentConfig(style={}, max_depth={}, trim_ws={}, final_nl={})",
            self.style, self.max_depth, self.trim_trailing_whitespace, self.final_newline
        )
    }
}

/// Builder for [`IndentConfig`].
#[derive(Debug, Clone)]
pub struct IndentConfigBuilder {
    config: IndentConfig,
}

impl IndentConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: IndentConfig::default(),
        }
    }

    pub fn style(mut self, style: IndentStyle) -> Self {
        self.config.style = style;
        self
    }

    pub fn max_depth(mut self, max: u32) -> Self {
        self.config.max_depth = max;
        self
    }

    pub fn trim_trailing_whitespace(mut self, trim: bool) -> Self {
        self.config.trim_trailing_whitespace = trim;
        self
    }

    pub fn final_newline(mut self, nl: bool) -> Self {
        self.config.final_newline = nl;
        self
    }

    /// Validate and build the configuration.
    pub fn build(self) -> Result<IndentConfig, IndentError> {
        if let IndentStyle::Spaces(w) = self.config.style {
            if w == 0 || w > 8 {
                return Err(IndentError::InvalidSpaceWidth(w));
            }
        }
        Ok(self.config)
    }
}

impl Default for IndentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Business-logic helpers
// ---------------------------------------------------------------------------

/// Statistics about indentation in a text buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentStats {
    pub total_lines: usize,
    pub blank_lines: usize,
    pub tab_indented_lines: usize,
    pub space_indented_lines: usize,
    pub max_indent_depth: u32,
    pub mixed: bool,
}

impl std::fmt::Display for IndentStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} lines ({} blank), tabs={}, spaces={}, max_depth={}, mixed={}",
            self.total_lines,
            self.blank_lines,
            self.tab_indented_lines,
            self.space_indented_lines,
            self.max_indent_depth,
            self.mixed,
        )
    }
}

/// Analyse the indentation present in `text`.
pub fn analyse_indentation(text: &str) -> IndentStats {
    let mut stats = IndentStats {
        total_lines: 0,
        blank_lines: 0,
        tab_indented_lines: 0,
        space_indented_lines: 0,
        max_indent_depth: 0,
        mixed: false,
    };

    for line in text.lines() {
        stats.total_lines += 1;
        if line.trim().is_empty() {
            stats.blank_lines += 1;
            continue;
        }
        let has_tab = line.starts_with('\t');
        let has_space = line.starts_with(' ');
        if has_tab {
            stats.tab_indented_lines += 1;
            let depth = line.bytes().take_while(|&b| b == b'\t').count() as u32;
            stats.max_indent_depth = stats.max_indent_depth.max(depth);
        }
        if has_space {
            stats.space_indented_lines += 1;
            let spaces = line.bytes().take_while(|&b| b == b' ').count() as u32;
            stats.max_indent_depth = stats.max_indent_depth.max(spaces / 4);
        }
    }

    if stats.tab_indented_lines > 0 && stats.space_indented_lines > 0 {
        stats.mixed = true;
    }
    stats
}

/// Validate that `text` uses only the given style, returning an error if mixed.
pub fn validate_indentation(text: &str, expected: IndentStyle) -> Result<(), IndentError> {
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match expected {
            IndentStyle::Tabs => {
                let leading_ws = &line[..line.len() - line.trim_start().len()];
                if leading_ws.contains(' ') {
                    return Err(IndentError::MixedIndentation);
                }
            }
            IndentStyle::Spaces(_) => {
                let leading_ws = &line[..line.len() - line.trim_start().len()];
                if leading_ws.contains('\t') {
                    return Err(IndentError::MixedIndentation);
                }
            }
        }
    }
    Ok(())
}

/// Apply an [`IndentConfig`] to reformat `text`.
///
/// This normalises indentation to the configured style, enforces max depth,
/// optionally trims trailing whitespace, and appends a final newline.
pub fn apply_config(text: &str, config: &IndentConfig) -> Result<String, IndentError> {
    let mut normalised = normalize_indentation(text, config.style);

    // Clamp indent depth
    let lines: Vec<&str> = normalised.lines().collect();
    let mut clamped = Vec::with_capacity(lines.len());
    for line in &lines {
        let depth = get_line_indent_level(line, config.style);
        if depth > config.max_depth {
            let trimmed = line.trim_start();
            let new_indent = config.style.indent_string_n(config.max_depth);
            clamped.push(format!("{}{}", new_indent, trimmed));
        } else {
            clamped.push(line.to_string());
        }
    }
    normalised = clamped.join("\n");

    if config.trim_trailing_whitespace {
        normalised = normalised
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");
    }

    if config.final_newline && !normalised.ends_with('\n') {
        normalised.push('\n');
    }

    Ok(normalised)
}

// ---------------------------------------------------------------------------
// Smart indentation (Enter key behavior)
// ---------------------------------------------------------------------------

/// Describes the indentation to apply when Enter is pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewlineIndent {
    /// Insert a newline and match the current line's indentation.
    SameLevel(String),
    /// Insert a newline with one extra indent level (line ends with open bracket).
    Deeper(String),
    /// Cursor is between brackets `{|}`: split into 3 lines.
    /// Fields: (indent_for_cursor_line, indent_for_closing_bracket_line)
    BetweenBrackets {
        cursor_indent: String,
        close_indent: String,
    },
}

/// Compute the indentation for a new line when Enter is pressed.
///
/// `lines` is the document split into lines, `line` is the 1-based line number
/// where the cursor is, `col` is the 1-based column of the cursor.
pub fn compute_indent_for_newline(
    lines: &[&str],
    line: u32,
    col: u32,
    style: IndentStyle,
) -> NewlineIndent {
    if line == 0 || line as usize > lines.len() {
        return NewlineIndent::SameLevel(String::new());
    }
    let current = lines[(line - 1) as usize];
    let current_indent = extract_leading_whitespace(current);
    let one_level = style.indent_string();

    // Text before and after cursor on the current line
    let col_idx = (col as usize).saturating_sub(1).min(current.len());
    let before = current[..col_idx].trim_end();
    let after = current[col_idx..].trim_start();

    let ends_with_open = before.ends_with('{')
        || before.ends_with('[')
        || before.ends_with('(');
    let starts_with_close = after.starts_with('}')
        || after.starts_with(']')
        || after.starts_with(')');

    if ends_with_open && starts_with_close {
        // Between brackets: split into 3 lines
        let cursor_indent = format!("{}{}", current_indent, one_level);
        let close_indent = current_indent.to_string();
        return NewlineIndent::BetweenBrackets {
            cursor_indent,
            close_indent,
        };
    }

    if ends_with_open {
        let deeper_indent = format!("{}{}", current_indent, one_level);
        return NewlineIndent::Deeper(deeper_indent);
    }

    // Check if next line starts with closing bracket (dedent case)
    if line < lines.len() as u32 {
        let next = lines[line as usize];
        let next_trimmed = next.trim_start();
        if next_trimmed.starts_with('}')
            || next_trimmed.starts_with(']')
            || next_trimmed.starts_with(')')
        {
            // Keep current indent (don't add extra)
            return NewlineIndent::SameLevel(current_indent.to_string());
        }
    }

    NewlineIndent::SameLevel(current_indent.to_string())
}

/// Extract leading whitespace from a line.
fn extract_leading_whitespace(line: &str) -> &str {
    let trimmed = line.trim_start();
    &line[..line.len() - trimmed.len()]
}

/// Adjust pasted text indentation to match the context at the cursor.
///
/// `base_indent` is the indentation of the line where the paste occurs.
pub fn auto_indent_on_paste(paste_text: &str, base_indent: &str, style: IndentStyle) -> String {
    let paste_lines: Vec<&str> = paste_text.lines().collect();
    if paste_lines.is_empty() {
        return String::new();
    }

    // Detect the indentation of the first non-empty line in paste
    let paste_indent_len = paste_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let ws = extract_leading_whitespace(l);
            indent_visual_width(ws, style)
        })
        .min()
        .unwrap_or(0);

    let base_width = indent_visual_width(base_indent, style);

    let mut result = Vec::with_capacity(paste_lines.len());
    for &line in paste_lines.iter() {
        if line.trim().is_empty() {
            result.push(String::new());
            continue;
        }
        let line_ws = extract_leading_whitespace(line);
        let line_width = indent_visual_width(line_ws, style);
        let relative = line_width.saturating_sub(paste_indent_len);
        let new_width = base_width + relative;
        let new_indent = build_indent(new_width, style);
        result.push(format!("{}{}", new_indent, line.trim_start()));
    }

    result.join("\n")
}

fn indent_visual_width(ws: &str, style: IndentStyle) -> u32 {
    let tab_width = match style {
        IndentStyle::Tabs => 4,
        IndentStyle::Spaces(n) => n,
    };
    ws.chars()
        .map(|c| if c == '\t' { tab_width } else { 1 })
        .sum()
}

fn build_indent(width: u32, style: IndentStyle) -> String {
    match style {
        IndentStyle::Tabs => {
            let tabs = width / 4;
            let spaces = width % 4;
            format!("{}{}", "\t".repeat(tabs as usize), " ".repeat(spaces as usize))
        }
        IndentStyle::Spaces(_) => " ".repeat(width as usize),
    }
}

/// Represents an indent guide (vertical line showing indent level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentGuide {
    /// The column position of this guide.
    pub column: u32,
    /// The starting line of this guide.
    pub start_line: usize,
    /// The ending line of this guide (inclusive).
    pub end_line: usize,
    /// The nesting depth (0 = outermost).
    pub depth: u32,
}

impl IndentGuide {
    /// Compute indent guides for a block of text.
    pub fn compute(text: &str, style: IndentStyle) -> Vec<IndentGuide> {
        let tab_width = match style {
            IndentStyle::Tabs => 4u32,
            IndentStyle::Spaces(n) => n,
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return Vec::new();
        }

        let indent_levels: Vec<u32> = lines
            .iter()
            .map(|line| {
                let trimmed = line.trim_start();
                if trimmed.is_empty() {
                    return 0;
                }
                let ws_len = line.len() - trimmed.len();
                let ws = &line[..ws_len];
                let width: u32 = ws
                    .chars()
                    .map(|c| if c == '\t' { tab_width } else { 1 })
                    .sum();
                width / tab_width
            })
            .collect();

        let mut guides = Vec::new();
        let max_depth = indent_levels.iter().copied().max().unwrap_or(0);

        for depth in 1..=max_depth {
            let column = depth * tab_width;
            let mut start: Option<usize> = None;
            for (i, &level) in indent_levels.iter().enumerate() {
                if level >= depth {
                    if start.is_none() {
                        start = Some(i);
                    }
                } else if let Some(s) = start {
                    guides.push(IndentGuide {
                        column,
                        start_line: s,
                        end_line: i - 1,
                        depth,
                    });
                    start = None;
                }
            }
            if let Some(s) = start {
                guides.push(IndentGuide {
                    column,
                    start_line: s,
                    end_line: lines.len() - 1,
                    depth,
                });
            }
        }
        guides
    }

    /// Number of lines this guide spans.
    pub fn line_span(&self) -> usize {
        self.end_line - self.start_line + 1
    }
}

impl fmt::Display for IndentGuide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Guide(col={}, lines={}-{}, depth={})",
            self.column, self.start_line, self.end_line, self.depth,
        )
    }
}

/// Auto-fixes indentation issues found in text.
pub struct IndentFixer;

impl IndentFixer {
    /// Fix mixed indentation by converting everything to the target style.
    pub fn fix_mixed(text: &str, target: IndentStyle) -> String {
        let tab_width = match target {
            IndentStyle::Tabs => 4u32,
            IndentStyle::Spaces(n) => n,
        };
        let mut result = String::with_capacity(text.len());
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                result.push_str(line);
                result.push('\n');
                continue;
            }
            let ws = &line[..line.len() - trimmed.len()];
            let width: u32 = ws
                .chars()
                .map(|c| if c == '\t' { tab_width } else { 1 })
                .sum();
            let new_ws = match target {
                IndentStyle::Tabs => {
                    let tabs = width / tab_width;
                    let spaces = width % tab_width;
                    format!(
                        "{}{}",
                        "\t".repeat(tabs as usize),
                        " ".repeat(spaces as usize),
                    )
                }
                IndentStyle::Spaces(_) => " ".repeat(width as usize),
            };
            result.push_str(&new_ws);
            result.push_str(trimmed);
            result.push('\n');
        }
        if !text.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        result
    }

    /// Remove trailing whitespace from all lines.
    pub fn trim_trailing(text: &str) -> String {
        text.lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A range of consecutive lines with the same indent level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentRange {
    pub start_line: usize,
    pub end_line: usize,
    pub indent_level: u32,
}

impl IndentRange {
    /// Compute indent ranges for a block of text.
    pub fn compute(text: &str, style: IndentStyle) -> Vec<IndentRange> {
        let tab_width = match style {
            IndentStyle::Tabs => 4u32,
            IndentStyle::Spaces(n) => n,
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return Vec::new();
        }

        let mut ranges = Vec::new();
        let mut current_level = u32::MAX;
        let mut start = 0usize;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let level = if trimmed.is_empty() {
                0
            } else {
                let ws = &line[..line.len() - trimmed.len()];
                let width: u32 = ws
                    .chars()
                    .map(|c| if c == '\t' { tab_width } else { 1 })
                    .sum();
                width / tab_width
            };
            if level != current_level {
                if current_level != u32::MAX {
                    ranges.push(IndentRange {
                        start_line: start,
                        end_line: i - 1,
                        indent_level: current_level,
                    });
                }
                current_level = level;
                start = i;
            }
        }
        if current_level != u32::MAX {
            ranges.push(IndentRange {
                start_line: start,
                end_line: lines.len() - 1,
                indent_level: current_level,
            });
        }
        ranges
    }

    /// Number of lines in this range.
    pub fn line_count(&self) -> usize {
        self.end_line - self.start_line + 1
    }
}

impl fmt::Display for IndentRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IndentRange(lines={}-{}, level={})",
            self.start_line, self.end_line, self.indent_level,
        )
    }
}

// ---------------------------------------------------------------------------
// Content-based indentation detection
// ---------------------------------------------------------------------------

/// Result of content-based indentation detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentDetection {
    pub style: IndentStyle,
    /// Confidence level: 0.0–1.0 (represented as 0–100 to avoid floats).
    pub confidence: u8,
    /// Number of indented lines analysed.
    pub sample_count: usize,
}

impl fmt::Display for IndentDetection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (confidence {}%, {} samples)", self.style, self.confidence, self.sample_count)
    }
}

/// Detect indentation from content with confidence scoring.
///
/// Unlike `detect_indentation`, this performs deeper analysis:
/// it looks at indent deltas between consecutive lines and votes
/// on the most likely indent unit size.
pub fn indentation_detect_from_content(text: &str) -> IndentDetection {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return IndentDetection { style: IndentStyle::Spaces(4), confidence: 0, sample_count: 0 };
    }

    let mut tab_lines = 0u32;
    let mut space_lines = 0u32;
    let mut delta_votes: [u32; 9] = [0; 9]; // index 1..=8
    let mut prev_indent: Option<usize> = None;
    let mut sample_count = 0usize;

    for line in &lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            prev_indent = None;
            continue;
        }

        let indent_part = &line[..line.len() - trimmed.len()];
        if indent_part.is_empty() {
            prev_indent = Some(0);
            continue;
        }

        sample_count += 1;

        if indent_part.starts_with('\t') {
            tab_lines += 1;
        } else {
            space_lines += 1;
            let width = indent_part.len();
            if let Some(prev) = prev_indent {
                let delta = if width > prev { width - prev } else { prev - width };
                if delta > 0 && delta <= 8 {
                    delta_votes[delta] += 1;
                }
            }
        }
        prev_indent = Some(indent_part.len());
    }

    if sample_count == 0 {
        return IndentDetection { style: IndentStyle::Spaces(4), confidence: 0, sample_count: 0 };
    }

    let total = tab_lines + space_lines;
    if tab_lines > space_lines {
        let confidence = if total > 0 { ((tab_lines as u64 * 100) / total as u64) as u8 } else { 0 };
        return IndentDetection { style: IndentStyle::Tabs, confidence, sample_count };
    }

    // Find the best space width
    let best_width = delta_votes[1..].iter()
        .enumerate()
        .max_by_key(|&(_, &v)| v)
        .map(|(i, _)| i + 1)
        .unwrap_or(4);

    let total_votes: u32 = delta_votes[1..].iter().sum();
    let best_votes = delta_votes[best_width];
    let confidence = if total_votes > 0 {
        ((best_votes as u64 * 100) / total_votes as u64).min(100) as u8
    } else if space_lines > 0 {
        50
    } else {
        0
    };

    IndentDetection {
        style: IndentStyle::Spaces(best_width as u32),
        confidence,
        sample_count,
    }
}


// ---------------------------------------------------------------------------
// IndentStyle helpers
// ---------------------------------------------------------------------------

impl IndentStyle {
    /// Returns the visual width of one indent level.
    pub fn width(&self) -> u32 {
        match self {
            IndentStyle::Spaces(n) => *n,
            IndentStyle::Tabs => 4, // conventional tab width
        }
    }

    /// Returns the character used for indentation.
    pub fn indent_char(&self) -> char {
        match self {
            IndentStyle::Spaces(_) => ' ',
            IndentStyle::Tabs => '\t',
        }
    }

    /// Common indent styles for selection UI.
    pub fn common_styles() -> Vec<IndentStyle> {
        vec![
            IndentStyle::Spaces(2),
            IndentStyle::Spaces(4),
            IndentStyle::Spaces(8),
            IndentStyle::Tabs,
        ]
    }
}

impl Default for IndentStyle {
    fn default() -> Self {
        IndentStyle::Spaces(4)
    }
}

// ---------------------------------------------------------------------------
// Indentation analysis
// ---------------------------------------------------------------------------

/// Counts the indentation level of a single line (alternate implementation
/// that counts all whitespace characters, not just leading contiguous ones).
pub fn line_indent_level(line: &str, style: IndentStyle) -> u32 {
    let ws = line.len() - line.trim_start().len();
    if ws == 0 {
        return 0;
    }
    let leading = &line[..ws];
    match style {
        IndentStyle::Tabs => leading.chars().filter(|&c| c == '\t').count() as u32,
        IndentStyle::Spaces(n) => {
            let spaces = leading.chars().filter(|&c| c == ' ').count() as u32;
            spaces / n
        }
    }
}

/// Returns the maximum indentation level in the text.
pub fn max_indent_level(text: &str, style: IndentStyle) -> u32 {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line_indent_level(line, style))
        .max()
        .unwrap_or(0)
}

/// Strip all leading whitespace from every line.
pub fn strip_all_indent(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

impl IndentStats {
    /// Analyze text for indentation statistics (delegates to [`analyse_indentation`]).
    pub fn analyze(text: &str) -> Self {
        analyse_indentation(text)
    }
}

// ---------------------------------------------------------------------------
// ReindentResult — adjust indentation to a target level
// ---------------------------------------------------------------------------

/// Result of a reindentation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReindentResult {
    /// The reindented text.
    pub text: String,
    /// Number of lines whose indentation was changed.
    pub lines_changed: usize,
    /// The target indent level that was applied.
    pub target_level: u32,
}

impl fmt::Display for ReindentResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReindentResult({} lines changed, target level {})",
            self.lines_changed, self.target_level,
        )
    }
}

/// Reindent all lines in `text` so that the minimum non-blank indent becomes
/// `target_level`, preserving relative indentation between lines.
pub fn reindent(text: &str, style: IndentStyle, target_level: u32) -> ReindentResult {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return ReindentResult {
            text: String::new(),
            lines_changed: 0,
            target_level,
        };
    }

    let min_level = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| get_line_indent_level(l, style))
        .min()
        .unwrap_or(0);

    let mut result_lines = Vec::with_capacity(lines.len());
    let mut changed = 0usize;

    for &line in &lines {
        if line.trim().is_empty() {
            result_lines.push(String::new());
            continue;
        }
        let current = get_line_indent_level(line, style);
        let new_level = current - min_level + target_level;
        let new_indent = style.indent_string_n(new_level);
        let trimmed = line.trim_start();
        let new_line = format!("{}{}", new_indent, trimmed);
        if new_line != line {
            changed += 1;
        }
        result_lines.push(new_line);
    }

    ReindentResult {
        text: result_lines.join("\n"),
        lines_changed: changed,
        target_level,
    }
}

impl From<ReindentResult> for String {
    fn from(r: ReindentResult) -> Self {
        r.text
    }
}

// ---------------------------------------------------------------------------
// IndentChange — diff indentation between old and new text
// ---------------------------------------------------------------------------

/// Describes an indentation change on a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentChange {
    /// Zero-based line number.
    pub line: usize,
    /// Indent level in the old text.
    pub old_level: u32,
    /// Indent level in the new text.
    pub new_level: u32,
}

impl IndentChange {
    /// Returns the signed delta (new - old) as i64.
    pub fn delta(&self) -> i64 {
        self.new_level as i64 - self.old_level as i64
    }

    /// Returns true if indentation increased.
    pub fn is_increase(&self) -> bool {
        self.new_level > self.old_level
    }

    /// Returns true if indentation decreased.
    pub fn is_decrease(&self) -> bool {
        self.new_level < self.old_level
    }
}

impl fmt::Display for IndentChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let arrow = if self.new_level > self.old_level {
            "→"
        } else {
            "←"
        };
        write!(
            f,
            "line {}: {} {} {}",
            self.line, self.old_level, arrow, self.new_level,
        )
    }
}

/// Compare two texts and return a list of lines where indentation changed.
///
/// Lines are compared pairwise up to the length of the shorter text.
/// Lines that exist only in one text are ignored.
pub fn detect_indent_changes(
    old_text: &str,
    new_text: &str,
    style: IndentStyle,
) -> Vec<IndentChange> {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let len = old_lines.len().min(new_lines.len());

    let mut changes = Vec::new();
    for i in 0..len {
        let old_level = get_line_indent_level(old_lines[i], style);
        let new_level = get_line_indent_level(new_lines[i], style);
        if old_level != new_level {
            changes.push(IndentChange {
                line: i,
                old_level,
                new_level,
            });
        }
    }
    changes
}

// ---------------------------------------------------------------------------
// Bulk indent-level extraction
// ---------------------------------------------------------------------------

/// Return the indent level for every line in `text`.
pub fn indent_levels(text: &str, style: IndentStyle) -> Vec<u32> {
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                0
            } else {
                get_line_indent_level(line, style)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Block indentation helpers
// ---------------------------------------------------------------------------

/// Apply a relative indent adjustment to a range of lines (0-indexed, inclusive).
/// Positive `delta` indents; negative dedents (clamping at 0).
pub fn adjust_indent_range(
    text: &str,
    start_line: usize,
    end_line: usize,
    delta: i32,
    style: IndentStyle,
) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::with_capacity(lines.len());
    for (i, &line) in lines.iter().enumerate() {
        if i >= start_line && i <= end_line && !line.trim().is_empty() {
            let current = get_line_indent_level(line, style);
            let new_level = if delta >= 0 {
                current + delta as u32
            } else {
                current.saturating_sub((-delta) as u32)
            };
            let trimmed = line.trim_start();
            result.push(format!("{}{}", style.indent_string_n(new_level), trimmed));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

/// Detect whether text uses consistent indentation (no mixing of tabs and spaces).
pub fn is_consistent(text: &str) -> bool {
    let stats = analyse_indentation(text);
    !stats.mixed
}

/// Count the number of indent transitions (level changes) in the text.
pub fn count_indent_transitions(text: &str, style: IndentStyle) -> usize {
    let levels = indent_levels(text, style);
    levels.windows(2).filter(|w| w[0] != w[1]).count()
}

/// Return the average indentation level across non-blank lines.
pub fn average_indent_level(text: &str, style: IndentStyle) -> f64 {
    let levels: Vec<u32> = text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| get_line_indent_level(l, style))
        .collect();
    if levels.is_empty() {
        return 0.0;
    }
    levels.iter().sum::<u32>() as f64 / levels.len() as f64
}

/// Extract a block of text at a specific indent level (contiguous lines at
/// that level or deeper, starting from the first match).
pub fn extract_block(text: &str, target_level: u32, style: IndentStyle) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut start = None;
    let mut end = None;
    for (i, &line) in lines.iter().enumerate() {
        let level = if line.trim().is_empty() { target_level } else { get_line_indent_level(line, style) };
        if level >= target_level && start.is_some() {
            end = Some(i);
        } else if level >= target_level && start.is_none() {
            start = Some(i);
            end = Some(i);
        } else if start.is_some() {
            break;
        }
    }
    match (start, end) {
        (Some(s), Some(e)) => Some(lines[s..=e].join("\n")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Indentation histogram
// ---------------------------------------------------------------------------

/// A histogram entry recording how many non-blank lines appear at each indent level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentHistogramEntry {
    pub level: u32,
    pub count: usize,
}

/// Compute a histogram of indent levels present in `text`.
///
/// Returns entries sorted by level (ascending). Levels with zero occurrences
/// between the minimum and maximum are included with `count = 0` so the
/// caller can render a contiguous bar chart.
pub fn indent_histogram(text: &str, style: IndentStyle) -> Vec<IndentHistogramEntry> {
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let level = get_line_indent_level(line, style);
        *counts.entry(level).or_insert(0) += 1;
    }

    if counts.is_empty() {
        return Vec::new();
    }

    let max_level = *counts.keys().last().unwrap();
    (0..=max_level)
        .map(|level| IndentHistogramEntry {
            level,
            count: counts.get(&level).copied().unwrap_or(0),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Foldable region detection
// ---------------------------------------------------------------------------

/// A foldable region detected by indentation: a line followed by one or more
/// lines at a deeper indent level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldRegion {
    /// Zero-based line number of the fold header.
    pub header_line: usize,
    /// Zero-based line number of the last line in the fold body (inclusive).
    pub end_line: usize,
    /// Indent level of the header line.
    pub indent_level: u32,
}

impl FoldRegion {
    /// Number of lines that would be hidden when folded (excludes header).
    pub fn hidden_lines(&self) -> usize {
        self.end_line - self.header_line
    }
}

/// Detect foldable regions based on indentation structure.
///
/// A fold starts at a line whose next non-blank line is indented deeper, and
/// extends until the indent level returns to the header's level or shallower.
pub fn detect_fold_regions(text: &str, style: IndentStyle) -> Vec<FoldRegion> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return Vec::new();
    }

    let levels: Vec<Option<u32>> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                None
            } else {
                Some(get_line_indent_level(l, style))
            }
        })
        .collect();

    let mut regions = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let header_level = match levels[i] {
            Some(l) => l,
            None => {
                i += 1;
                continue;
            }
        };

        // Find next non-blank line
        let mut j = i + 1;
        while j < lines.len() && levels[j].is_none() {
            j += 1;
        }
        if j >= lines.len() {
            break;
        }

        let next_level = levels[j].unwrap();
        if next_level <= header_level {
            i += 1;
            continue;
        }

        // Extend the fold region until we return to header level or shallower
        let mut end = j;
        let mut k = j + 1;
        while k < lines.len() {
            match levels[k] {
                None => {}
                Some(l) if l > header_level => {
                    end = k;
                }
                Some(_) => break,
            }
            k += 1;
        }

        regions.push(FoldRegion {
            header_line: i,
            end_line: end,
            indent_level: header_level,
        });

        i = end + 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Indent/dedent selected line ranges (by line numbers)
// ---------------------------------------------------------------------------

/// Indent specific lines (0-indexed) by one level, leaving others unchanged.
pub fn indent_selected_lines(text: &str, selected: &[usize], style: IndentStyle) -> String {
    let one = style.indent_string();
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::with_capacity(lines.len());
    for (i, &line) in lines.iter().enumerate() {
        if selected.contains(&i) && !line.trim().is_empty() {
            result.push(format!("{}{}", one, line));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

/// Dedent specific lines (0-indexed) by one level, leaving others unchanged.
/// Lines with no indentation are left as-is.
pub fn dedent_selected_lines(text: &str, selected: &[usize], style: IndentStyle) -> String {
    let strip = match style {
        IndentStyle::Tabs => 1usize,
        IndentStyle::Spaces(n) => n as usize,
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::with_capacity(lines.len());
    for (i, &line) in lines.iter().enumerate() {
        if selected.contains(&i) {
            let current = get_line_indent_level(line, style);
            if current > 0 {
                result.push(line[strip..].to_string());
            } else {
                result.push(line.to_string());
            }
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

// ---------------------------------------------------------------------------
// Mixed indentation diagnostics
// ---------------------------------------------------------------------------

/// A diagnostic for a line with mixed tabs and spaces in its leading whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedIndentDiagnostic {
    /// Zero-based line number.
    pub line: usize,
    /// Number of tab characters in leading whitespace.
    pub tabs: usize,
    /// Number of space characters in leading whitespace.
    pub spaces: usize,
}

/// Scan text and return diagnostics for every line whose leading whitespace
/// contains *both* tabs and spaces (within the same line).
pub fn find_mixed_indent_lines(text: &str) -> Vec<MixedIndentDiagnostic> {
    let mut diags = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let ws = &line[..line.len() - line.trim_start().len()];
        if ws.is_empty() {
            continue;
        }
        let tabs = ws.chars().filter(|&c| c == '\t').count();
        let spaces = ws.chars().filter(|&c| c == ' ').count();
        if tabs > 0 && spaces > 0 {
            diags.push(MixedIndentDiagnostic { line: i, tabs, spaces });
        }
    }
    diags
}

// ---------------------------------------------------------------------------
// Whitespace-visible rendering
// ---------------------------------------------------------------------------

/// Render whitespace characters visibly for debugging/display purposes.
///
/// Tabs are replaced with `→` followed by spaces to the next tab stop, and
/// trailing spaces on each line are replaced with `·`.
pub fn render_whitespace_visible(text: &str, tab_width: u32) -> String {
    let mut result = Vec::new();
    for line in text.lines() {
        let trimmed_end = line.trim_end();
        let trailing_count = line.len() - trimmed_end.len();

        let mut visible = String::with_capacity(line.len() * 2);
        let mut col = 0u32;
        for ch in trimmed_end.chars() {
            if ch == '\t' {
                visible.push('→');
                col += 1;
                let fill = tab_width.saturating_sub(col % tab_width);
                // Fill to next tab stop (minus the arrow itself if at boundary)
                let pad = if col % tab_width == 0 {
                    tab_width - 1
                } else {
                    fill - 1
                };
                for _ in 0..pad {
                    visible.push(' ');
                    col += 1;
                }
            } else {
                visible.push(ch);
                col += 1;
            }
        }
        for _ in 0..trailing_count {
            visible.push('·');
        }
        result.push(visible);
    }
    result.join("\n")
}

// ---------------------------------------------------------------------------
// Ensure final newline / strip final newline
// ---------------------------------------------------------------------------

/// Ensure `text` ends with exactly one newline character.
pub fn ensure_final_newline(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Strip all trailing newlines from `text`.
pub fn strip_final_newlines(text: &str) -> &str {
    text.trim_end_matches('\n')
}

// ---------------------------------------------------------------------------
// Re-tab / un-tab operations
// ---------------------------------------------------------------------------

/// Convert leading spaces to tabs in every line, using the given tab width.
/// Only converts complete groups of `tab_width` spaces; leftover spaces remain.
pub fn retab(text: &str, tab_width: u32) -> String {
    if tab_width == 0 {
        return text.to_string();
    }
    let tw = tab_width as usize;
    let mut result = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let ws = &line[..line.len() - trimmed.len()];
        // Count only spaces (tabs pass through)
        let space_count = ws.chars().filter(|&c| c == ' ').count();
        let existing_tabs = ws.chars().filter(|&c| c == '\t').count();
        let new_tabs = existing_tabs + space_count / tw;
        let remaining_spaces = space_count % tw;
        let new_ws = format!(
            "{}{}",
            "\t".repeat(new_tabs),
            " ".repeat(remaining_spaces),
        );
        result.push(format!("{}{}", new_ws, trimmed));
    }
    result.join("\n")
}

/// Convert leading tabs to spaces in every line, using the given tab width.
pub fn untab(text: &str, tab_width: u32) -> String {
    let tw = tab_width as usize;
    let mut result = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let ws = &line[..line.len() - trimmed.len()];
        let tab_count = ws.chars().filter(|&c| c == '\t').count();
        let space_count = ws.chars().filter(|&c| c == ' ').count();
        let total_spaces = tab_count * tw + space_count;
        result.push(format!("{}{}", " ".repeat(total_spaces), trimmed));
    }
    result.join("\n")
}


// ---------------------------------------------------------------------------
// IndentationGuideRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IndentationGuideRenderer {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl IndentationGuideRenderer {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for IndentationGuideRenderer {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for IndentationGuideRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "IndentationGuideRenderer({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// IndentationFixAll
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IndentationFixAll {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl IndentationFixAll {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for IndentationFixAll {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for IndentationFixAll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "IndentationFixAll({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// IndentationGuideRendererSnapshot — point-in-time snapshot of IndentationGuideRenderer state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IndentationGuideRendererSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl IndentationGuideRendererSnapshot {
    pub fn capture(source: &IndentationGuideRenderer, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for IndentationGuideRendererSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// IndentationFixAllStats — aggregate statistics for IndentationFixAll
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct IndentationFixAllStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl IndentationFixAllStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for IndentationFixAllStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// IndentationGuideRendererConfig — configuration for IndentationGuideRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IndentationGuideRendererConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl IndentationGuideRendererConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for IndentationGuideRendererConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for IndentationGuideRendererConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// IndentationStatistics
// ---------------------------------------------------------------------------

/// Analyzes indentation statistics for a body of text.
pub struct IndentationStatistics {
    level_counts: HashMap<u32, usize>,
    tab_lines: usize,
    space_lines: usize,
    total_lines: usize,
}

impl IndentationStatistics {
    pub fn analyze(text: &str) -> Self {
        let mut level_counts: HashMap<u32, usize> = HashMap::new();
        let mut tab_lines = 0usize;
        let mut space_lines = 0usize;
        let mut total_lines = 0usize;

        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            total_lines += 1;
            if line.starts_with('\t') {
                tab_lines += 1;
                let tabs = line.len() - line.trim_start_matches('\t').len();
                *level_counts.entry(tabs as u32).or_insert(0) += 1;
            } else {
                let spaces = line.len() - line.trim_start_matches(' ').len();
                if spaces > 0 {
                    space_lines += 1;
                }
                *level_counts.entry(spaces as u32).or_insert(0) += 1;
            }
        }

        Self { level_counts, tab_lines, space_lines, total_lines }
    }

    pub fn most_common_indent(&self) -> u32 {
        self.level_counts
            .iter()
            .filter(|&(&k, _)| k > 0)
            .max_by_key(|&(_, &v)| v)
            .map(|(&k, _)| k)
            .unwrap_or(0)
    }

    pub fn is_mixed(&self) -> bool {
        self.tab_lines > 0 && self.space_lines > 0
    }

    pub fn tab_vs_space_ratio(&self) -> f64 {
        if self.space_lines == 0 {
            return if self.tab_lines > 0 { f64::INFINITY } else { 0.0 };
        }
        self.tab_lines as f64 / self.space_lines as f64
    }

    pub fn recommended_style(&self) -> IndentStyle {
        if self.tab_lines > self.space_lines {
            IndentStyle::Tabs
        } else {
            IndentStyle::Spaces(self.most_common_indent().max(2))
        }
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    pub fn lines_at_indent(&self, level: u32) -> usize {
        self.level_counts.get(&level).copied().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// IndentGuideRenderer
// ---------------------------------------------------------------------------

/// Computes guide positions for indent levels.
pub struct IndentGuideRenderer {
    indent_size: u32,
}

impl IndentGuideRenderer {
    pub fn new(indent_size: u32) -> Self {
        Self { indent_size: indent_size.max(1) }
    }

    /// Return columns where guides should be drawn for a given nesting level.
    pub fn guide_columns(&self, max_level: u32) -> Vec<u32> {
        (1..=max_level).map(|l| l * self.indent_size).collect()
    }

    /// Return guides visible in a column range.
    pub fn visible_guides_in_range(&self, max_level: u32, col_start: u32, col_end: u32) -> Vec<u32> {
        self.guide_columns(max_level)
            .into_iter()
            .filter(|&c| c >= col_start && c <= col_end)
            .collect()
    }

    /// Return the guide column at a given position, if any.
    pub fn guide_at_column(&self, col: u32) -> bool {
        col > 0 && col % self.indent_size == 0
    }
}

// ---------------------------------------------------------------------------
// IndentationFixer
// ---------------------------------------------------------------------------

/// Normalizes and fixes indentation issues.
pub struct IndentationFixer;

impl IndentationFixer {
    /// Convert all leading tabs to spaces.
    pub fn convert_all_to_spaces(text: &str, tab_size: u32) -> String {
        let spaces: String = std::iter::repeat(' ').take(tab_size as usize).collect();
        text.lines()
            .map(|line| {
                let leading_tabs = line.len() - line.trim_start_matches('\t').len();
                if leading_tabs > 0 {
                    let rest = &line[leading_tabs..];
                    format!("{}{}", spaces.repeat(leading_tabs), rest)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Convert all leading spaces to tabs.
    pub fn convert_all_to_tabs(text: &str, tab_size: u32) -> String {
        let ts = tab_size.max(1) as usize;
        text.lines()
            .map(|line| {
                let leading_spaces = line.len() - line.trim_start_matches(' ').len();
                if leading_spaces > 0 {
                    let tabs = leading_spaces / ts;
                    let remainder = leading_spaces % ts;
                    let rest = &line[leading_spaces..];
                    let mut s = "\t".repeat(tabs);
                    for _ in 0..remainder {
                        s.push(' ');
                    }
                    s.push_str(rest);
                    s
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Remove trailing whitespace from each line.
    pub fn fix_trailing_whitespace(text: &str) -> String {
        text.lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Remove indentation from blank lines.
    pub fn trim_blank_line_indent(text: &str) -> String {
        text.lines()
            .map(|line| {
                if line.trim().is_empty() { "" } else { line }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}


/// Configuration manager for indentation functionality.
pub struct IndentationConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl IndentationConfig {
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

    pub fn merge(&mut self, other: &IndentationConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for indentation operations.
pub struct IndentationRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl IndentationRateTracker {
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

/// Validation result collector for indentation.
pub struct IndentationValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl IndentationValidator {
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

    pub fn merge(&mut self, other: &IndentationValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Indentation detection and conversion — extended utilities (qp)
// ---------------------------------------------------------------------------

/// Metric accumulator for indent operations.
#[derive(Debug, Clone)]
pub struct QpMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QpMetrics {
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

/// Sliding-window rate counter for indent.
#[derive(Debug, Clone)]
pub struct QpRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QpRateWindow {
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

/// A small LRU-style cache for indent lookups.
#[derive(Debug, Clone)]
pub struct QpLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QpLruCache {
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
// xb_ utilities – batch 3
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer3 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer3 {
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
pub fn xb_fnv1a_3(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_3<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_3<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_3(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_3(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 92
// ---------------------------------------------------------------------------

/// Generic object pool `Xc92Pool<T>`.
pub struct Xc92Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc92Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc92PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc92Pool<T> {
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
    pub fn stats(&self) -> Xc92PoolStats {
        Xc92PoolStats {
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

impl<T> Default for Xc92Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc92Scheduler`.
pub struct Xc92Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc92Scheduler {
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

impl Default for Xc92Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_92 hash for the given byte slice.
pub fn xc_92_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_92 convention.
pub fn xc_92_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe4 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe4Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe4PipelineError {
    pub stage: Xe4Stage,
    pub message: String,
}

impl std::fmt::Display for Xe4PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe4Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe4Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError>>>,
    stage_names: Vec<Xe4Stage>,
}

impl Xe4Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe4Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe4Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe4Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe4Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
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

    pub fn compose(mut self, other: Xe4Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe4CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe4CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe4Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe4CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe4CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe4Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe4CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_4_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe4CacheEntry {
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

    fn xe_4_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe4CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_4_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
    Ok(data)
}

pub fn xe_4_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_4_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_4_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_4_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe4PipelineError> {
    Err(Xe4PipelineError {
        stage: Xe4Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #67
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf67Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf67TrieNode {
    children: std::collections::HashMap<char, Xf67TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf67Trie {
    root: Xf67TrieNode,
    count: usize,
}

impl Xf67Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf67TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf67TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf67TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf67BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf67BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 91).
pub struct Xh91SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh91SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 133 as u64,
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

/// A compact bit set supporting boolean operations (variant 91).
pub struct Xh91BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh91BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 91).
pub struct Xi91Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi91Deque<T> {
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
pub struct Xi91Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi91Interval {
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

/// A simple interval tree (variant 91).
pub struct Xi91IntervalTree {
    xi_intervals: Vec<Xi91Interval>,
}

impl Xi91IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi91Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi91Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi91Interval) -> Vec<&Xi91Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi91Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi91Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi91Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi91Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi91Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi91Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 91) ---

/// Disjoint set / union-find for crate 91.
pub struct Xj91UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj91UnionFind {
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

const XJ91_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 91.
pub struct Xj91BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj91BTreeNode<K, V>>>,
    len: usize,
}

struct Xj91BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj91BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj91BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ91_BTREE_ORDER - 1
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
        let mid = XJ91_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj91BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj91BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj91BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj91BTreeNode::xj_new_leaf();
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
    fn detect_spaces() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n        nested();\n}\n";
        let style = detect_indentation(text);
        // Should detect spaces (not tabs) — exact size may vary
        assert!(matches!(style, IndentStyle::Spaces(_)));
    }

    #[test]
    fn detect_tabs() {
        let text = "fn main() {\n\tlet x = 1;\n\tlet y = 2;\n}\n";
        assert_eq!(detect_indentation(text), IndentStyle::Tabs);
    }

    #[test]
    fn convert_tabs_to_spaces() {
        let input = "\tline1\n\t\tline2";
        let result = convert_indentation(input, IndentStyle::Tabs, IndentStyle::Spaces(4));
        assert_eq!(result, "    line1\n        line2");
    }

    #[test]
    fn convert_spaces_to_tabs() {
        let input = "    line1\n        line2";
        let result = convert_indentation(input, IndentStyle::Spaces(4), IndentStyle::Tabs);
        assert_eq!(result, "\tline1\n\t\tline2");
    }

    #[test]
    fn display_indent_style() {
        assert_eq!(format!("{}", IndentStyle::Spaces(4)), "Spaces(4)");
        assert_eq!(format!("{}", IndentStyle::Tabs), "Tabs");
    }

    #[test]
    fn indent_string_methods() {
        assert_eq!(IndentStyle::Spaces(2).indent_string(), "  ");
        assert_eq!(IndentStyle::Tabs.indent_string(), "\t");
        assert_eq!(IndentStyle::Spaces(4).indent_string_n(3), "            ");
        assert_eq!(IndentStyle::Tabs.indent_string_n(2), "\t\t");
    }

    #[test]
    fn line_indent_level() {
        assert_eq!(get_line_indent_level("\t\tcode", IndentStyle::Tabs), 2);
        assert_eq!(get_line_indent_level("        code", IndentStyle::Spaces(4)), 2);
        assert_eq!(get_line_indent_level("code", IndentStyle::Spaces(4)), 0);
        assert_eq!(get_line_indent_level("   code", IndentStyle::Spaces(4)), 0);
    }

    #[test]
    fn indent_and_dedent() {
        let text = "line1\n  line2\n\n  line3";
        let indented = indent_lines(text, IndentStyle::Spaces(2), 1);
        assert_eq!(indented, "  line1\n    line2\n\n    line3");

        let back = dedent_lines(&indented, IndentStyle::Spaces(2), 1);
        assert_eq!(back, "line1\n  line2\n\n  line3");
    }

    #[test]
    fn normalize_tabs_to_spaces() {
        let input = "fn main() {\n\tlet x = 1;\n\t\tnested();\n}\n";
        let result = normalize_indentation(input, IndentStyle::Spaces(4));
        assert_eq!(result, "fn main() {\n    let x = 1;\n        nested();\n}");
    }

    // --- new tests ---

    #[test]
    fn indent_style_spaces_validates_width() {
        assert!(IndentStyle::spaces(4).is_ok());
        assert!(IndentStyle::spaces(1).is_ok());
        assert!(IndentStyle::spaces(8).is_ok());
        assert_eq!(IndentStyle::spaces(0), Err(IndentError::InvalidSpaceWidth(0)));
        assert_eq!(IndentStyle::spaces(9), Err(IndentError::InvalidSpaceWidth(9)));
    }

    #[test]
    fn indent_style_queries() {
        assert!(IndentStyle::Spaces(2).is_spaces());
        assert!(!IndentStyle::Spaces(2).is_tabs());
        assert!(IndentStyle::Tabs.is_tabs());
        assert!(!IndentStyle::Tabs.is_spaces());
    }

    #[test]
    fn visual_width() {
        assert_eq!(IndentStyle::Spaces(2).visual_width(), 2);
        assert_eq!(IndentStyle::Tabs.visual_width(), 4);
    }

    #[test]
    fn indent_config_builder_defaults() {
        let cfg = IndentConfigBuilder::new().build().unwrap();
        assert_eq!(cfg.style, IndentStyle::Spaces(4));
        assert_eq!(cfg.max_depth, 20);
        assert!(cfg.trim_trailing_whitespace);
        assert!(cfg.final_newline);
    }

    #[test]
    fn indent_config_builder_custom() {
        let cfg = IndentConfigBuilder::new()
            .style(IndentStyle::Tabs)
            .max_depth(10)
            .trim_trailing_whitespace(false)
            .final_newline(false)
            .build()
            .unwrap();
        assert_eq!(cfg.style, IndentStyle::Tabs);
        assert_eq!(cfg.max_depth, 10);
        assert!(!cfg.trim_trailing_whitespace);
        assert!(!cfg.final_newline);
    }

    #[test]
    fn indent_config_builder_rejects_bad_width() {
        let result = IndentConfigBuilder::new()
            .style(IndentStyle::Spaces(0))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn analyse_indentation_spaces() {
        let text = "fn f() {\n    a;\n        b;\n\n    c;\n}\n";
        let stats = analyse_indentation(text);
        assert_eq!(stats.total_lines, 6);
        assert_eq!(stats.blank_lines, 1);
        assert_eq!(stats.tab_indented_lines, 0);
        assert_eq!(stats.space_indented_lines, 3);
        assert!(!stats.mixed);
    }

    #[test]
    fn analyse_indentation_mixed() {
        let text = "\tline1\n    line2\n";
        let stats = analyse_indentation(text);
        assert!(stats.mixed);
        assert_eq!(stats.tab_indented_lines, 1);
        assert_eq!(stats.space_indented_lines, 1);
    }

    #[test]
    fn validate_indentation_ok() {
        let text = "fn f() {\n    a;\n    b;\n}\n";
        assert!(validate_indentation(text, IndentStyle::Spaces(4)).is_ok());
    }

    #[test]
    fn validate_indentation_mixed_error() {
        let text = "\tline1\n    line2\n";
        assert_eq!(
            validate_indentation(text, IndentStyle::Tabs),
            Err(IndentError::MixedIndentation)
        );
    }

    #[test]
    fn apply_config_trims_and_adds_newline() {
        let cfg = IndentConfig {
            style: IndentStyle::Spaces(4),
            max_depth: 20,
            trim_trailing_whitespace: true,
            final_newline: true,
        };
        let input = "hello   \nworld  ";
        let result = apply_config(input, &cfg).unwrap();
        assert!(result.ends_with('\n'));
        assert!(!result.contains("   \n"));
    }

    #[test]
    fn apply_config_clamps_depth() {
        let cfg = IndentConfig {
            style: IndentStyle::Spaces(2),
            max_depth: 2,
            trim_trailing_whitespace: false,
            final_newline: false,
        };
        let input = "      deeply_nested"; // 6 spaces = 3 levels at width 2
        let result = apply_config(input, &cfg).unwrap();
        // max depth 2 → 4 leading spaces
        assert_eq!(result, "    deeply_nested");
    }

    #[test]
    fn indent_error_display() {
        let e = IndentError::InvalidSpaceWidth(0);
        assert!(format!("{}", e).contains("invalid space width 0"));
        let e2 = IndentError::MaxDepthExceeded { depth: 25, max: 20 };
        assert!(format!("{}", e2).contains("25"));
        let e3 = IndentError::MixedIndentation;
        assert!(format!("{}", e3).contains("mixed"));
    }

    #[test]
    fn indent_config_display() {
        let cfg = IndentConfig::default();
        let s = format!("{}", cfg);
        assert!(s.contains("Spaces(4)"));
        assert!(s.contains("max_depth=20"));
    }

    // -- Smart indentation tests --------------------------------------------

    #[test]
    fn newline_same_level() {
        let lines = vec!["    let x = 1;", "    let y = 2;"];
        let result = compute_indent_for_newline(&lines, 1, 15, IndentStyle::Spaces(4));
        assert_eq!(result, NewlineIndent::SameLevel("    ".into()));
    }

    #[test]
    fn newline_deeper_after_open_brace() {
        let lines = vec!["fn main() {", ""];
        let result = compute_indent_for_newline(&lines, 1, 12, IndentStyle::Spaces(4));
        assert_eq!(result, NewlineIndent::Deeper("    ".into()));
    }

    #[test]
    fn newline_between_brackets() {
        let lines = vec!["fn main() {}"];
        // Cursor between { and } (col 12)
        let result = compute_indent_for_newline(&lines, 1, 12, IndentStyle::Spaces(4));
        assert_eq!(result, NewlineIndent::BetweenBrackets {
            cursor_indent: "    ".into(),
            close_indent: "".into(),
        });
    }

    #[test]
    fn newline_deeper_after_open_paren() {
        let lines = vec!["    foo("];
        let result = compute_indent_for_newline(&lines, 1, 9, IndentStyle::Spaces(4));
        assert_eq!(result, NewlineIndent::Deeper("        ".into()));
    }

    #[test]
    fn auto_indent_paste_adjusts() {
        let paste = "    line1\n        line2\n    line3";
        let result = auto_indent_on_paste(paste, "  ", IndentStyle::Spaces(4));
        assert_eq!(result, "  line1\n      line2\n  line3");
    }

    #[test]
    fn indent_guides_basic() {
        let text = "fn main() {\n    let x = 1;\n    if true {\n        inner();\n    }\n}\n";
        let guides = IndentGuide::compute(text, IndentStyle::Spaces(4));
        assert!(!guides.is_empty());
        let depth1: Vec<_> = guides.iter().filter(|g| g.depth == 1).collect();
        assert!(!depth1.is_empty());
        assert!(depth1[0].start_line >= 1);
    }

    #[test]
    fn indent_guides_empty() {
        let guides = IndentGuide::compute("", IndentStyle::Spaces(4));
        assert!(guides.is_empty());
    }

    #[test]
    fn indent_fixer_mixed() {
        let text = "\tline1\n    line2\n\t    line3";
        let fixed = IndentFixer::fix_mixed(text, IndentStyle::Spaces(4));
        // All indentation should be spaces
        for line in fixed.lines() {
            let ws = &line[..line.len() - line.trim_start().len()];
            assert!(!ws.contains('\t'), "found tab in: {line:?}");
        }
    }

    #[test]
    fn indent_fixer_trim_trailing() {
        let text = "hello   \nworld  \nfoo";
        let trimmed = IndentFixer::trim_trailing(text);
        assert_eq!(trimmed, "hello\nworld\nfoo");
    }

    #[test]
    fn indent_ranges_basic() {
        let text = "top\n    indented\n    also\n        deep\ntop_again";
        let ranges = IndentRange::compute(text, IndentStyle::Spaces(4));
        assert!(ranges.len() >= 3);
        assert_eq!(ranges[0].indent_level, 0);
        assert_eq!(ranges[1].indent_level, 1);
    }

    #[test]
    fn indent_guide_display() {
        let guide = IndentGuide {
            column: 4,
            start_line: 1,
            end_line: 5,
            depth: 1,
        };
        assert_eq!(format!("{guide}"), "Guide(col=4, lines=1-5, depth=1)");
        assert_eq!(guide.line_span(), 5);
    }

    // -- indentation_detect_from_content --

    #[test]
    fn detect_content_spaces_4() {
        let text = "fn main() {\n    let x = 1;\n    if true {\n        inner();\n    }\n}\n";
        let d = indentation_detect_from_content(text);
        assert_eq!(d.style, IndentStyle::Spaces(4));
        assert!(d.confidence > 50);
    }

    #[test]
    fn detect_content_spaces_2() {
        let text = "function f() {\n  let x = 1;\n  if (true) {\n    inner();\n  }\n}\n";
        let d = indentation_detect_from_content(text);
        assert_eq!(d.style, IndentStyle::Spaces(2));
    }

    #[test]
    fn detect_content_tabs() {
        let text = "fn main() {\n\tlet x = 1;\n\tif true {\n\t\tinner();\n\t}\n}\n";
        let d = indentation_detect_from_content(text);
        assert_eq!(d.style, IndentStyle::Tabs);
        assert!(d.confidence > 50);
    }

    #[test]
    fn detect_content_empty() {
        let d = indentation_detect_from_content("");
        assert_eq!(d.confidence, 0);
        assert_eq!(d.sample_count, 0);
    }

    #[test]
    fn detect_content_no_indent() {
        let text = "line1\nline2\nline3\n";
        let d = indentation_detect_from_content(text);
        assert_eq!(d.sample_count, 0);
    }

    #[test]
    fn indent_detection_display() {
        let d = IndentDetection { style: IndentStyle::Spaces(4), confidence: 85, sample_count: 20 };
        let s = format!("{d}");
        assert!(s.contains("85%"));
        assert!(s.contains("20 samples"));
    }

    #[test]
    fn test_indent_style_width() {
        assert_eq!(IndentStyle::Spaces(2).width(), 2);
        assert_eq!(IndentStyle::Tabs.width(), 4);
    }

    #[test]
    fn test_indent_style_string() {
        assert_eq!(IndentStyle::Spaces(2).indent_string(), "  ");
        assert_eq!(IndentStyle::Tabs.indent_string(), "\t");
    }

    #[test]
    fn test_indent_style_is_spaces_tabs() {
        assert!(IndentStyle::Spaces(4).is_spaces());
        assert!(!IndentStyle::Spaces(4).is_tabs());
        assert!(IndentStyle::Tabs.is_tabs());
    }

    #[test]
    fn test_indent_style_display() {
        assert_eq!(format!("{}", IndentStyle::Spaces(2)), "Spaces(2)");
        assert_eq!(format!("{}", IndentStyle::Tabs), "Tabs");
    }

    #[test]
    fn test_indent_style_default() {
        assert_eq!(IndentStyle::default(), IndentStyle::Spaces(4));
    }

    #[test]
    fn test_indent_style_common() {
        let common = IndentStyle::common_styles();
        assert_eq!(common.len(), 4);
        assert!(common.contains(&IndentStyle::Tabs));
    }

    #[test]
    fn test_line_indent_level_fn() {
        assert_eq!(super::line_indent_level("    hello", IndentStyle::Spaces(4)), 1);
        assert_eq!(super::line_indent_level("        hello", IndentStyle::Spaces(4)), 2);
        assert_eq!(super::line_indent_level("\thello", IndentStyle::Tabs), 1);
        assert_eq!(super::line_indent_level("hello", IndentStyle::Spaces(4)), 0);
    }

    #[test]
    fn test_max_indent_level() {
        let text = "a\n    b\n        c\n";
        assert_eq!(max_indent_level(text, IndentStyle::Spaces(4)), 2);
    }

    #[test]
    fn test_indent_stats_analyze() {
        let text = "top\n    indented\n\n        deep\n";
        let stats = IndentStats::analyze(text);
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.space_indented_lines, 2);
        assert_eq!(stats.blank_lines, 1);
        assert!(!stats.mixed);
        assert!(format!("{stats}").contains("4 lines"));
    }

    #[test]
    fn test_indent_stats_mixed() {
        let text = "    spaces\n\ttabs\n";
        let stats = IndentStats::analyze(text);
        assert!(stats.mixed);
    }

    #[test]
    fn test_strip_all_indent() {
        let text = "  a\n    b\nc\n";
        assert_eq!(strip_all_indent(text), "a\nb\nc");
    }

    // -- reindent tests -----------------------------------------------------

    #[test]
    fn reindent_shifts_to_target_level() {
        let text = "    fn inner() {\n        body();\n    }";
        let r = reindent(text, IndentStyle::Spaces(4), 0);
        assert_eq!(r.text, "fn inner() {\n    body();\n}");
        assert_eq!(r.lines_changed, 3);
        assert_eq!(r.target_level, 0);
    }

    #[test]
    fn reindent_increases_indent() {
        let text = "a\n    b\n        c";
        let r = reindent(text, IndentStyle::Spaces(4), 2);
        assert_eq!(r.text, "        a\n            b\n                c");
        assert_eq!(r.lines_changed, 3);
    }

    #[test]
    fn reindent_preserves_blank_lines() {
        let text = "    a\n\n    b";
        let r = reindent(text, IndentStyle::Spaces(4), 0);
        assert_eq!(r.text, "a\n\nb");
        assert_eq!(r.lines_changed, 2); // blank lines don't count
    }

    #[test]
    fn reindent_empty_text() {
        let r = reindent("", IndentStyle::Spaces(4), 3);
        assert_eq!(r.text, "");
        assert_eq!(r.lines_changed, 0);
    }

    #[test]
    fn reindent_result_display() {
        let r = ReindentResult {
            text: "hello".into(),
            lines_changed: 5,
            target_level: 2,
        };
        let s = format!("{r}");
        assert!(s.contains("5 lines changed"));
        assert!(s.contains("target level 2"));
    }

    #[test]
    fn reindent_result_into_string() {
        let r = reindent("    hello", IndentStyle::Spaces(4), 0);
        let s: String = r.into();
        assert_eq!(s, "hello");
    }

    // -- detect_indent_changes tests ----------------------------------------

    #[test]
    fn detect_changes_identifies_modified_lines() {
        let old = "fn main() {\n    a();\n    b();\n}";
        let new = "fn main() {\n        a();\n    b();\n}";
        let changes = detect_indent_changes(old, new, IndentStyle::Spaces(4));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].line, 1);
        assert_eq!(changes[0].old_level, 1);
        assert_eq!(changes[0].new_level, 2);
        assert!(changes[0].is_increase());
        assert!(!changes[0].is_decrease());
        assert_eq!(changes[0].delta(), 1);
    }

    #[test]
    fn detect_changes_empty_when_identical() {
        let text = "a\n    b\n        c";
        let changes = detect_indent_changes(text, text, IndentStyle::Spaces(4));
        assert!(changes.is_empty());
    }

    #[test]
    fn detect_changes_decrease() {
        let old = "        deep";
        let new = "    shallow";
        let changes = detect_indent_changes(old, new, IndentStyle::Spaces(4));
        assert_eq!(changes.len(), 1);
        assert!(changes[0].is_decrease());
        assert_eq!(changes[0].delta(), -1);
    }

    #[test]
    fn indent_change_display() {
        let c = IndentChange { line: 3, old_level: 1, new_level: 2 };
        let s = format!("{c}");
        assert!(s.contains("line 3"));
        assert!(s.contains("→"));
    }

    // -- indent_levels tests ------------------------------------------------

    #[test]
    fn indent_levels_returns_per_line_levels() {
        let text = "a\n    b\n        c\n\n    d";
        let levels = indent_levels(text, IndentStyle::Spaces(4));
        assert_eq!(levels, vec![0, 1, 2, 0, 1]);
    }

    // -- adjust_indent_range tests ------------------------------------------

    #[test]
    fn adjust_indent_range_increases_indent() {
        let text = "fn main() {\nlet x = 1;\nlet y = 2;\n}";
        let result = adjust_indent_range(text, 1, 2, 1, IndentStyle::Spaces(4));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "fn main() {");
        assert_eq!(lines[1], "    let x = 1;");
        assert_eq!(lines[2], "    let y = 2;");
        assert_eq!(lines[3], "}");
    }

    #[test]
    fn adjust_indent_range_decreases_indent() {
        let text = "fn main() {\n        let x = 1;\n        let y = 2;\n}";
        let result = adjust_indent_range(text, 1, 2, -1, IndentStyle::Spaces(4));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[1], "    let x = 1;");
        assert_eq!(lines[2], "    let y = 2;");
    }

    #[test]
    fn is_consistent_detects_clean_indentation() {
        assert!(is_consistent("    a\n    b\n        c"));
        assert!(is_consistent("\ta\n\t\tb"));
        assert!(!is_consistent("\ta\n    b"));
    }

    #[test]
    fn count_indent_transitions_counts_changes() {
        let text = "a\n    b\n    c\n        d\na";
        let transitions = count_indent_transitions(text, IndentStyle::Spaces(4));
        assert_eq!(transitions, 3); // 0→1, 1→2, 2→0
    }

    #[test]
    fn average_indent_level_computes_mean() {
        let text = "a\n    b\n        c";
        let avg = average_indent_level(text, IndentStyle::Spaces(4));
        assert!((avg - 1.0).abs() < 0.01); // (0 + 1 + 2) / 3 = 1.0
    }

    #[test]
    fn extract_block_finds_indented_block() {
        let text = "top\n    inner1\n    inner2\nbottom";
        let block = extract_block(text, 1, IndentStyle::Spaces(4));
        assert_eq!(block, Some("    inner1\n    inner2".to_string()));
    }

    // -- indent_histogram tests ---------------------------------------------

    #[test]
    fn histogram_basic() {
        let text = "a\n    b\n    c\n        d\na";
        let hist = indent_histogram(text, IndentStyle::Spaces(4));
        assert_eq!(hist.len(), 3); // levels 0, 1, 2
        assert_eq!(hist[0].level, 0);
        assert_eq!(hist[0].count, 2); // "a" and "a"
        assert_eq!(hist[1].level, 1);
        assert_eq!(hist[1].count, 2); // "b" and "c"
        assert_eq!(hist[2].level, 2);
        assert_eq!(hist[2].count, 1); // "d"
    }

    #[test]
    fn histogram_empty() {
        let hist = indent_histogram("", IndentStyle::Spaces(4));
        assert!(hist.is_empty());
    }

    #[test]
    fn histogram_fills_gaps() {
        // Level 0 and level 3, levels 1 and 2 should appear with count 0
        let text = "a\n            deep";
        let hist = indent_histogram(text, IndentStyle::Spaces(4));
        assert_eq!(hist.len(), 4); // 0, 1, 2, 3
        assert_eq!(hist[1].count, 0);
        assert_eq!(hist[2].count, 0);
        assert_eq!(hist[3].count, 1);
    }

    // -- fold region tests --------------------------------------------------

    #[test]
    fn fold_regions_basic() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\nfn other() {\n    body();\n}";
        let regions = detect_fold_regions(text, IndentStyle::Spaces(4));
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].header_line, 0);
        assert_eq!(regions[0].end_line, 2);
        assert_eq!(regions[0].hidden_lines(), 2);
        assert_eq!(regions[1].header_line, 4);
    }

    #[test]
    fn fold_regions_empty() {
        assert!(detect_fold_regions("", IndentStyle::Spaces(4)).is_empty());
        assert!(detect_fold_regions("single", IndentStyle::Spaces(4)).is_empty());
    }

    #[test]
    fn fold_regions_nested() {
        let text = "a\n    b\n        c\n    d\ne";
        let regions = detect_fold_regions(text, IndentStyle::Spaces(4));
        // "a" folds over "b", "c", "d"
        assert!(!regions.is_empty());
        assert_eq!(regions[0].header_line, 0);
        assert_eq!(regions[0].end_line, 3);
    }

    // -- indent/dedent selected lines tests ---------------------------------

    #[test]
    fn indent_selected_lines_basic() {
        let text = "aaa\nbbb\nccc\nddd";
        let result = indent_selected_lines(text, &[1, 2], IndentStyle::Spaces(4));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "aaa");
        assert_eq!(lines[1], "    bbb");
        assert_eq!(lines[2], "    ccc");
        assert_eq!(lines[3], "ddd");
    }

    #[test]
    fn dedent_selected_lines_basic() {
        let text = "aaa\n    bbb\n    ccc\nddd";
        let result = dedent_selected_lines(text, &[1, 2], IndentStyle::Spaces(4));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "aaa");
        assert_eq!(lines[1], "bbb");
        assert_eq!(lines[2], "ccc");
        assert_eq!(lines[3], "ddd");
    }

    #[test]
    fn dedent_selected_noop_at_zero() {
        let text = "aaa\nbbb";
        let result = dedent_selected_lines(text, &[0, 1], IndentStyle::Spaces(4));
        assert_eq!(result, "aaa\nbbb");
    }

    // -- mixed indent diagnostics tests -------------------------------------

    #[test]
    fn find_mixed_indent_lines_detects_mixed() {
        let text = "clean\n\t  mixed\n    spaces\n\ttabs";
        let diags = find_mixed_indent_lines(text);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 1);
        assert_eq!(diags[0].tabs, 1);
        assert_eq!(diags[0].spaces, 2);
    }

    #[test]
    fn find_mixed_indent_lines_clean() {
        let text = "    a\n    b\n        c";
        assert!(find_mixed_indent_lines(text).is_empty());
    }

    // -- whitespace rendering tests -----------------------------------------

    #[test]
    fn render_whitespace_trailing_dots() {
        let text = "hello   ";
        let rendered = render_whitespace_visible(text, 4);
        assert!(rendered.contains("hello···"));
    }

    #[test]
    fn render_whitespace_tabs() {
        let text = "\thello";
        let rendered = render_whitespace_visible(text, 4);
        assert!(rendered.starts_with('→'));
    }

    // -- final newline tests ------------------------------------------------

    #[test]
    fn ensure_final_newline_adds_when_missing() {
        assert_eq!(ensure_final_newline("hello"), "hello\n");
    }

    #[test]
    fn ensure_final_newline_normalizes_multiple() {
        assert_eq!(ensure_final_newline("hello\n\n\n"), "hello\n");
    }

    #[test]
    fn strip_final_newlines_removes_all() {
        assert_eq!(strip_final_newlines("hello\n\n\n"), "hello");
        assert_eq!(strip_final_newlines("hello"), "hello");
    }

    // -- retab / untab tests ------------------------------------------------

    #[test]
    fn retab_converts_spaces_to_tabs() {
        let text = "        line1\n    line2\n  partial";
        let result = retab(text, 4);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "\t\tline1");
        assert_eq!(lines[1], "\tline2");
        assert_eq!(lines[2], "  partial"); // 2 spaces < 4, stays
    }

    #[test]
    fn untab_converts_tabs_to_spaces() {
        let text = "\t\tline1\n\tline2";
        let result = untab(text, 4);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "        line1");
        assert_eq!(lines[1], "    line2");
    }

    #[test]
    fn retab_zero_width_noop() {
        let text = "    hello";
        assert_eq!(retab(text, 0), text);
    }

    #[test]
    fn retab_untab_roundtrip() {
        let original = "\t\tdeep\n\tshallow\ntop";
        let spaced = untab(original, 4);
        let back = retab(&spaced, 4);
        assert_eq!(back, original);
    }

    #[test] fn indentationGuideRenderer_new() { let s = IndentationGuideRenderer::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn indentationGuideRenderer_add() { let mut s = IndentationGuideRenderer::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn indentationGuideRenderer_remove() { let mut s = IndentationGuideRenderer::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn indentationGuideRenderer_config() { let mut s = IndentationGuideRenderer::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn indentationGuideRenderer_nav() { let mut s = IndentationGuideRenderer::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn indentationGuideRenderer_filter() { let mut s = IndentationGuideRenderer::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn indentationGuideRenderer_display() { assert!(format!("{}", IndentationGuideRenderer::new()).contains("IndentationGuideRenderer")); }
    #[test] fn indentationFixAll_new() { let s = IndentationFixAll::new(); assert!(s.is_empty()); }
    #[test] fn indentationFixAll_add() { let mut s = IndentationFixAll::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn indentationFixAll_active() { let mut s = IndentationFixAll::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn indentationFixAll_error() { let mut s = IndentationFixAll::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn indentationFixAll_rm_group() { let mut s = IndentationFixAll::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn indentationFixAll_display() { assert!(format!("{}", IndentationFixAll::new()).contains("IndentationFixAll")); }


    #[test] fn indentationGuideRenderer_snap_capture() {
        let s = IndentationGuideRenderer::new();
        let snap = IndentationGuideRendererSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn indentationGuideRenderer_snap_stale() {
        let s = IndentationGuideRenderer::new();
        let snap = IndentationGuideRendererSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn indentationGuideRenderer_snap_diff() {
        let s = IndentationGuideRenderer::new();
        let s1v = IndentationGuideRendererSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn indentationGuideRenderer_snap_display() {
        let s = IndentationGuideRenderer::new();
        let snap = IndentationGuideRendererSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn indentationFixAll_stats_record() {
        let mut st = IndentationFixAllStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn indentationFixAll_stats_hit_ratio() {
        let mut st = IndentationFixAllStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn indentationFixAll_stats_merge() {
        let mut a = IndentationFixAllStats::new();
        a.total_adds = 5;
        let mut b = IndentationFixAllStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn indentationFixAll_stats_display() {
        let st = IndentationFixAllStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn indentationGuideRenderer_config_default() {
        let c = IndentationGuideRendererConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn indentationGuideRenderer_config_builder() {
        let c = IndentationGuideRendererConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn indentationGuideRenderer_config_labels() {
        let mut c = IndentationGuideRendererConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn indentationGuideRenderer_config_cleanup_threshold() {
        let c = IndentationGuideRendererConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn indentationGuideRenderer_config_display() {
        assert!(format!("{}", IndentationGuideRendererConfig::new()).contains("Config"));
    }
    #[test] fn indentationFixAll_stats_peaks() {
        let mut st = IndentationFixAllStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- IndentationStatistics tests --

    #[test]
    fn stats_pure_spaces() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        let stats = IndentationStatistics::analyze(text);
        assert!(!stats.is_mixed());
        assert_eq!(stats.recommended_style(), IndentStyle::Spaces(4));
    }

    #[test]
    fn stats_pure_tabs() {
        let text = "fn main() {\n\tlet x = 1;\n\tlet y = 2;\n}\n";
        let stats = IndentationStatistics::analyze(text);
        assert!(!stats.is_mixed());
        assert_eq!(stats.recommended_style(), IndentStyle::Tabs);
    }

    #[test]
    fn stats_mixed() {
        let text = "\tline1\n    line2\n";
        let stats = IndentationStatistics::analyze(text);
        assert!(stats.is_mixed());
    }

    #[test]
    fn stats_most_common_indent() {
        let text = "a\n  b\n  c\n    d\n";
        let stats = IndentationStatistics::analyze(text);
        assert_eq!(stats.most_common_indent(), 2);
    }

    #[test]
    fn stats_total_lines() {
        let text = "a\nb\n\nc\n";
        let stats = IndentationStatistics::analyze(text);
        assert_eq!(stats.total_lines(), 3);
    }

    #[test]
    fn stats_lines_at_indent() {
        let text = "a\n  b\n  c\n";
        let stats = IndentationStatistics::analyze(text);
        assert_eq!(stats.lines_at_indent(2), 2);
        assert_eq!(stats.lines_at_indent(0), 1);
    }

    // -- IndentGuideRenderer tests --

    #[test]
    fn guide_columns() {
        let r = IndentGuideRenderer::new(4);
        assert_eq!(r.guide_columns(3), vec![4, 8, 12]);
    }

    #[test]
    fn visible_guides_in_range() {
        let r = IndentGuideRenderer::new(4);
        let g = r.visible_guides_in_range(4, 5, 12);
        assert_eq!(g, vec![8, 12]);
    }

    #[test]
    fn guide_at_column() {
        let r = IndentGuideRenderer::new(4);
        assert!(r.guide_at_column(8));
        assert!(!r.guide_at_column(5));
        assert!(!r.guide_at_column(0));
    }

    // -- IndentationFixer tests --

    #[test]
    fn fixer_tabs_to_spaces() {
        let text = "\tline1\n\t\tline2";
        let result = IndentationFixer::convert_all_to_spaces(text, 4);
        assert!(result.starts_with("    line1"));
        assert!(result.contains("        line2"));
    }

    #[test]
    fn fixer_spaces_to_tabs() {
        let text = "    line1\n        line2";
        let result = IndentationFixer::convert_all_to_tabs(text, 4);
        assert!(result.starts_with("\tline1"));
        assert!(result.contains("\t\tline2"));
    }

    #[test]
    fn fixer_trailing_whitespace() {
        let text = "hello   \nworld  ";
        let result = IndentationFixer::fix_trailing_whitespace(text);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn fixer_trim_blank_lines() {
        let text = "hello\n    \nworld";
        let result = IndentationFixer::trim_blank_line_indent(text);
        assert_eq!(result, "hello\n\nworld");
    }


    #[test]
    fn indentation_config_new() {
        let cfg = IndentationConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn indentation_config_set_get() {
        let mut cfg = IndentationConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn indentation_config_remove() {
        let mut cfg = IndentationConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn indentation_config_keys_sorted() {
        let mut cfg = IndentationConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn indentation_config_bump_version() {
        let mut cfg = IndentationConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn indentation_config_clear() {
        let mut cfg = IndentationConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn indentation_config_merge() {
        let mut cfg1 = IndentationConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = IndentationConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn indentation_config_disable() {
        let mut cfg = IndentationConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn indentation_rate_tracker_empty() {
        let rt = IndentationRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn indentation_rate_tracker_record() {
        let mut rt = IndentationRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn indentation_rate_tracker_prune() {
        let mut rt = IndentationRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn indentation_validator_valid() {
        let v = IndentationValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn indentation_validator_errors() {
        let mut v = IndentationValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn indentation_validator_clear() {
        let mut v = IndentationValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn indentation_validator_merge() {
        let mut v1 = IndentationValidator::new();
        v1.add_error("e1");
        let mut v2 = IndentationValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn indentation_rate_tracker_clear() {
        let mut rt = IndentationRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qp_metrics_empty() {
        let m = QpMetrics::new("indent");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qp_metrics_record_and_mean() {
        let mut m = QpMetrics::new("indent");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qp_metrics_min_max() {
        let mut m = QpMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qp_metrics_variance_and_std() {
        let mut m = QpMetrics::new("v");
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
    fn qp_metrics_percentile() {
        let mut m = QpMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qp_metrics_merge() {
        let mut a = QpMetrics::new("a");
        a.record(1.0);
        let mut b = QpMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qp_metrics_reset() {
        let mut m = QpMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qp_rate_window_empty() {
        let rw = QpRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qp_rate_window_tick_and_rate() {
        let mut rw = QpRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qp_lru_cache_basic() {
        let mut c = QpLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qp_lru_cache_contains_and_keys() {
        let mut c = QpLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qp_lru_cache_remove() {
        let mut c = QpLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qp_metrics_sum() {
        let mut m = QpMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qp_metrics_label() {
        let m = QpMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qp_lru_cache_clear() {
        let mut c = QpLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_3_push_and_len() {
        let mut rb = super::XbRingBuffer3::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_3_overwrite() {
        let mut rb = super::XbRingBuffer3::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_3_get_out_of_bounds() {
        let rb = super::XbRingBuffer3::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_3_drain_all() {
        let mut rb = super::XbRingBuffer3::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_3_peek_front_back() {
        let mut rb = super::XbRingBuffer3::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_3_clear() {
        let mut rb = super::XbRingBuffer3::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_3_capacity() {
        let rb = super::XbRingBuffer3::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_3_basic() {
        let h = super::xb_fnv1a_3(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_3(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_3_different_inputs() {
        let h1 = super::xb_fnv1a_3(b"abc");
        let h2 = super::xb_fnv1a_3(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_3_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_3(&data);
        let dec = super::xb_rle_decode_3(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_3_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_3(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_3(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_3_values() {
        assert!((super::xb_clamp_3(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_3(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_3(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_3_values() {
        assert!((super::xb_lerp_3(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_3(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_3(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_3_wrap_around_twice() {
        let mut rb = super::XbRingBuffer3::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 92 ----

    #[test]
    fn xc_92_pool_new_empty() {
        let pool: super::Xc92Pool<i32> = super::Xc92Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_92_pool_release_acquire() {
        let mut pool = super::Xc92Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_92_pool_acquire_empty() {
        let mut pool: super::Xc92Pool<i32> = super::Xc92Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_92_pool_full() {
        let mut pool = super::Xc92Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_92_pool_drain() {
        let mut pool = super::Xc92Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_92_pool_stats() {
        let mut pool = super::Xc92Pool::new(8);
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
    fn xc_92_pool_clear() {
        let mut pool = super::Xc92Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_92_pool_shrink() {
        let mut pool = super::Xc92Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_92_pool_default() {
        let pool: super::Xc92Pool<String> = super::Xc92Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_92_pool_extend() {
        let mut pool = super::Xc92Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_92_pool_retain() {
        let mut pool = super::Xc92Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_92_scheduler_round_robin() {
        let mut sched = super::Xc92Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_92_scheduler_empty() {
        let mut sched = super::Xc92Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_92_scheduler_reset() {
        let mut sched = super::Xc92Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_92_scheduler_add_remove() {
        let mut sched = super::Xc92Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_92_scheduler_targets() {
        let sched = super::Xc92Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_92_hash_empty() {
        assert_eq!(super::xc_92_hash(b""), 5381);
    }

    #[test]
    fn xc_92_hash_data() {
        let h = super::xc_92_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_92_hash(b"hello"), h);
    }

    #[test]
    fn xc_92_reverse_str() {
        assert_eq!(super::xc_92_reverse("abc"), "cba");
        assert_eq!(super::xc_92_reverse(""), "");
    }


    #[test]
    fn xe_4_pipeline_empty() {
        let p = super::Xe4Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_4_pipeline_parse_stage() {
        let p = super::Xe4Pipeline::new()
            .add_parse(super::xe_4_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_4_pipeline_transform_double() {
        let p = super::Xe4Pipeline::new()
            .add_transform(super::xe_4_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_4_pipeline_validate_reverse() {
        let p = super::Xe4Pipeline::new()
            .add_validate(super::xe_4_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_4_pipeline_emit_filter() {
        let p = super::Xe4Pipeline::new()
            .add_emit(super::xe_4_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_4_pipeline_multi_stage() {
        let p = super::Xe4Pipeline::new()
            .add_parse(super::xe_4_pipeline_identity)
            .add_transform(super::xe_4_pipeline_double)
            .add_validate(super::xe_4_pipeline_reverse)
            .add_emit(super::xe_4_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_4_pipeline_error_propagation() {
        let p = super::Xe4Pipeline::new()
            .add_parse(super::xe_4_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe4Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_4_pipeline_compose() {
        let p1 = super::Xe4Pipeline::new()
            .add_parse(super::xe_4_pipeline_identity);
        let p2 = super::Xe4Pipeline::new()
            .add_transform(super::xe_4_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_4_pipeline_error_display() {
        let e = super::Xe4PipelineError {
            stage: super::Xe4Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_4_cache_put_get() {
        let mut c = super::Xe4Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_4_cache_miss() {
        let mut c: super::Xe4Cache<&str, i32> = super::Xe4Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_4_cache_ttl_expiry() {
        let mut c = super::Xe4Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_4_cache_evict() {
        let mut c = super::Xe4Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_4_cache_capacity() {
        let mut c = super::Xe4Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_4_cache_stats() {
        let mut c = super::Xe4Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_4_cache_clear() {
        let mut c = super::Xe4Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #67 --

    #[test]
    fn xf67_trie_insert_search() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf67_trie_starts_with() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf67_trie_remove() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf67_trie_word_count() {
        let mut t = Xf67Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf67_trie_longest_prefix() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf67_trie_all_words() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf67_trie_autocomplete() {
        let mut t = Xf67Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf67_trie_empty_search() {
        let t = Xf67Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf67_bloom_add_contains() {
        let mut bf = Xf67BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf67_bloom_probably_absent() {
        let bf = Xf67BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf67_bloom_false_positive_rate() {
        let mut bf = Xf67BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf67_bloom_clear() {
        let mut bf = Xf67BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf67_bloom_union() {
        let mut a = Xf67BloomFilter::xf_new(512, 2);
        let mut b = Xf67BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf67_bloom_intersection_estimate() {
        let mut a = Xf67BloomFilter::xf_new(512, 2);
        let mut b = Xf67BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf67_bloom_union_size_mismatch() {
        let a = Xf67BloomFilter::xf_new(256, 2);
        let b = Xf67BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh91_skip_insert_contains() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh91_skip_remove() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh91_skip_len() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh91_skip_range_query() {
        let mut sl = super::Xh91SkipList::xh_new(4);
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
    fn xh91_skip_floor_ceiling() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh91_skip_rank() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh91_skip_empty() {
        let sl = super::Xh91SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh91_skip_duplicates() {
        let mut sl = super::Xh91SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh91_bitset_set_test() {
        let mut bs = super::Xh91BitSet::xh_new(256);
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
    fn xh91_bitset_clear_count() {
        let mut bs = super::Xh91BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh91_bitset_and_or_xor() {
        let mut a = super::Xh91BitSet::xh_new(128);
        let mut b = super::Xh91BitSet::xh_new(128);
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
    fn xh91_bitset_iter_ones() {
        let mut bs = super::Xh91BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh91_bitset_first_last() {
        let mut bs = super::Xh91BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh91_bitset_empty() {
        let bs = super::Xh91BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi91_deque_push_pop_back() {
        let mut dq = super::Xi91Deque::xi_new(4);
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
    fn xi91_deque_push_pop_front() {
        let mut dq = super::Xi91Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi91_deque_mixed_ops() {
        let mut dq = super::Xi91Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi91_deque_get_and_split() {
        let mut dq = super::Xi91Deque::xi_new(8);
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
    fn xi91_deque_rotate_left() {
        let mut dq = super::Xi91Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi91_deque_rotate_right() {
        let mut dq = super::Xi91Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi91_deque_grow() {
        let mut dq = super::Xi91Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi91_deque_empty() {
        let dq = super::Xi91Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi91_interval_tree_insert_query() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi91Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi91Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi91_interval_tree_overlap() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi91Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi91Interval::xi_new(12, 20));
        let q = super::Xi91Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi91_interval_tree_remove() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi91Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi91_interval_tree_gaps() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi91Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi91Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi91Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi91Interval::xi_new(8, 10));
    }

    #[test]
    fn xi91_interval_tree_merge() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi91Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi91Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi91Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi91Interval::xi_new(10, 15));
    }

    #[test]
    fn xi91_interval_tree_all() {
        let mut tree = super::Xi91IntervalTree::xi_new();
        tree.xi_insert(super::Xi91Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi91Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi91_interval_tree_empty() {
        let tree = super::Xi91IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi91_interval_tree_contains_point() {
        let iv = super::Xi91Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 91) ---

    #[test]
    fn xj_91_uf_make_and_find() {
        let mut uf = super::Xj91UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_91_uf_union_connected() {
        let mut uf = super::Xj91UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_91_uf_component_count() {
        let mut uf = super::Xj91UnionFind::xj_new();
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
    fn xj_91_uf_component_size() {
        let mut uf = super::Xj91UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_91_uf_largest_component() {
        let mut uf = super::Xj91UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_91_uf_many_elements() {
        let mut uf = super::Xj91UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_91_uf_separate_components() {
        let mut uf = super::Xj91UnionFind::xj_new();
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
    fn xj_91_uf_path_compression() {
        let mut uf = super::Xj91UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_91_bt_insert_get() {
        let mut bt = super::Xj91BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_91_bt_contains_len() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_91_bt_replace() {
        let mut bt = super::Xj91BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_91_bt_remove() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_91_bt_keys_values() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_91_bt_range() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_91_bt_min_max() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_91_bt_many_inserts() {
        let mut bt = super::Xj91BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
