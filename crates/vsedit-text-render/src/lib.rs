//! Unicode-aware terminal text rendering.
//!
//! Equivalent to VS Code's text rendering pipeline, adapted for terminal output.
//! Handles wide characters, tab expansion, and control characters.

use unicode_width::UnicodeWidthChar;

/// Result of rendering a line for terminal display.
#[derive(Debug, Clone)]
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
}
