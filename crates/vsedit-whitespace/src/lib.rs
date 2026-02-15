//! Whitespace visualization and trimming.

/// Trim trailing whitespace from all lines.
pub fn trim_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ensure file ends with exactly one newline.
pub fn ensure_final_newline(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Trim final empty lines.
pub fn trim_final_newlines(text: &str) -> String {
    let mut result = text.trim_end().to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Count trailing whitespace characters on a line.
pub fn trailing_whitespace_count(line: &str) -> usize {
    line.len() - line.trim_end().len()
}

/// Replace tabs with spaces.
pub fn tabs_to_spaces(text: &str, tab_size: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut col = 0;
    for ch in text.chars() {
        if ch == '\t' {
            let spaces = tab_size - (col % tab_size);
            for _ in 0..spaces {
                result.push(' ');
            }
            col += spaces;
        } else if ch == '\n' {
            result.push(ch);
            col = 0;
        } else {
            result.push(ch);
            col += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_trailing() {
        assert_eq!(trim_trailing_whitespace("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn final_newline() {
        assert_eq!(ensure_final_newline("hello"), "hello\n");
        assert_eq!(ensure_final_newline("hello\n\n"), "hello\n");
    }

    #[test]
    fn trim_final() {
        assert_eq!(trim_final_newlines("hello\n\n\n"), "hello\n");
    }

    #[test]
    fn trailing_count() {
        assert_eq!(trailing_whitespace_count("hello   "), 3);
        assert_eq!(trailing_whitespace_count("hello"), 0);
    }

    #[test]
    fn tab_conversion() {
        assert_eq!(tabs_to_spaces("\thello", 4), "    hello");
        assert_eq!(tabs_to_spaces("ab\tcd", 4), "ab  cd");
    }
}
