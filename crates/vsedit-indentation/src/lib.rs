//! Indentation detection and manipulation.

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
}
