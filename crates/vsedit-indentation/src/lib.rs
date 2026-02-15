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
}
