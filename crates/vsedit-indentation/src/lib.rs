//! Indentation detection and manipulation.

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
}
