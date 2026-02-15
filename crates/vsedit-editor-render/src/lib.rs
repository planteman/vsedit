//! Terminal editor line rendering.

/// A rendered editor line for terminal output.
#[derive(Debug, Clone)]
pub struct RenderedEditorLine {
    pub line_number: u32,
    pub content: String,
    pub decorations: Vec<LineDecoration>,
}

/// A decoration on a rendered line.
#[derive(Debug, Clone)]
pub struct LineDecoration {
    pub start_col: u32,
    pub end_col: u32,
    pub kind: DecorationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    Selection,
    SearchMatch,
    CurrentSearchMatch,
    BracketMatch,
    WordHighlight,
    Error,
    Warning,
    Info,
    Hint,
}

/// The editor renderer.
pub struct EditorRenderer {
    pub viewport_start_line: u32,
    pub viewport_height: u32,
    pub line_number_width: u16,
}

impl EditorRenderer {
    pub fn new() -> Self {
        Self { viewport_start_line: 1, viewport_height: 24, line_number_width: 4 }
    }

    pub fn line_number_width_for(line_count: u32) -> u16 {
        let digits = if line_count == 0 { 1 } else { (line_count as f64).log10() as u16 + 1 };
        digits.max(2) + 1 // extra space
    }
}

impl Default for EditorRenderer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_number_width() {
        assert_eq!(EditorRenderer::line_number_width_for(1), 3);
        assert_eq!(EditorRenderer::line_number_width_for(99), 3);
        assert_eq!(EditorRenderer::line_number_width_for(100), 4);
        assert_eq!(EditorRenderer::line_number_width_for(9999), 5);
    }
}
