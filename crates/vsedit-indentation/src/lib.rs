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
}
