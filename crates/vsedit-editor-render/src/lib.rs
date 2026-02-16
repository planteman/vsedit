//! Terminal editor line rendering.
//!
//! Provides viewport-aware rendering of editor lines with decoration merging
//! for selections, search highlights, diagnostics, and other visual markers.

/// A rendered editor line for terminal output.
#[derive(Debug, Clone)]
pub struct RenderedEditorLine {
    pub line_number: u32,
    pub content: String,
    pub decorations: Vec<LineDecoration>,
    pub is_current_line: bool,
    pub is_wrapped: bool,
}

/// A decoration on a rendered line.
#[derive(Debug, Clone)]
pub struct LineDecoration {
    pub start_col: u32,
    pub end_col: u32,
    pub kind: DecorationKind,
}

/// All supported decoration kinds for the editor renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    FoldedRegion,
    InlayHint,
    GitGutterAdd,
    GitGutterModify,
    GitGutterDelete,
    Ruler,
    WhitespaceChar,
    IndentGuide,
    TrailingWhitespace,
}

/// Priority for decoration layering (higher = rendered on top).
impl DecorationKind {
    pub fn priority(&self) -> u8 {
        match self {
            Self::IndentGuide => 1,
            Self::WhitespaceChar => 2,
            Self::TrailingWhitespace => 3,
            Self::Ruler => 4,
            Self::GitGutterAdd | Self::GitGutterModify | Self::GitGutterDelete => 5,
            Self::FoldedRegion => 6,
            Self::InlayHint => 7,
            Self::WordHighlight => 10,
            Self::BracketMatch => 11,
            Self::SearchMatch => 12,
            Self::CurrentSearchMatch => 13,
            Self::Hint => 14,
            Self::Info => 15,
            Self::Warning => 16,
            Self::Error => 17,
            Self::Selection => 20,
        }
    }
}

/// Viewport state tracking.
#[derive(Debug, Clone)]
pub struct ViewportState {
    pub first_visible_line: u32,
    pub last_visible_line: u32,
    pub scroll_top: u32,
    pub height: u32,
    pub total_lines: u32,
}

impl ViewportState {
    pub fn new(height: u32) -> Self {
        Self {
            first_visible_line: 1,
            last_visible_line: height,
            scroll_top: 0,
            height,
            total_lines: 0,
        }
    }

    /// Update viewport after a scroll or content change.
    pub fn update(&mut self, first_line: u32, total_lines: u32) {
        self.total_lines = total_lines;
        self.first_visible_line = first_line.max(1).min(total_lines.max(1));
        self.last_visible_line = (self.first_visible_line + self.height - 1).min(total_lines);
        self.scroll_top = self.first_visible_line.saturating_sub(1);
    }

    /// Whether the given line is within the visible viewport.
    pub fn is_visible(&self, line: u32) -> bool {
        line >= self.first_visible_line && line <= self.last_visible_line
    }

    /// Number of lines actually visible (may be less than height near end of file).
    pub fn visible_line_count(&self) -> u32 {
        if self.last_visible_line >= self.first_visible_line {
            self.last_visible_line - self.first_visible_line + 1
        } else {
            0
        }
    }
}

/// Cursor display information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorDisplay {
    pub line: u32,
    pub column: u32,
    pub is_visible: bool,
    pub style: CursorStyle,
}

/// Cursor visual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Line,
    Underline,
}

/// The editor renderer with full viewport state.
pub struct EditorRenderer {
    pub viewport: ViewportState,
    pub line_number_width: u16,
    pub cursor_style: CursorStyle,
    pub show_current_line_highlight: bool,
}

impl EditorRenderer {
    pub fn new() -> Self {
        Self {
            viewport: ViewportState::new(24),
            line_number_width: 4,
            cursor_style: CursorStyle::Block,
            show_current_line_highlight: true,
        }
    }

    /// Compute the minimum width needed for line numbers.
    pub fn line_number_width_for(line_count: u32) -> u16 {
        let digits = if line_count == 0 { 1 } else { (line_count as f64).log10() as u16 + 1 };
        digits.max(2) + 1
    }

    /// Produce rendered lines for the current viewport.
    pub fn render_viewport(&self, lines: &[&str], cursor_line: u32) -> Vec<RenderedEditorLine> {
        let mut result = Vec::new();
        let start = self.viewport.first_visible_line as usize;
        let end = self.viewport.last_visible_line as usize;

        for i in start..=end {
            if i == 0 || i > lines.len() {
                continue;
            }
            let content = lines[i - 1].to_string();
            result.push(RenderedEditorLine {
                line_number: i as u32,
                content,
                decorations: Vec::new(),
                is_current_line: i as u32 == cursor_line,
                is_wrapped: false,
            });
        }
        result
    }

    /// Merge decorations into rendered lines, sorted by priority.
    pub fn compute_decorations(
        rendered: &mut [RenderedEditorLine],
        decorations: &[LineDecoration],
        viewport_start: u32,
    ) {
        for dec in decorations {
            for line in rendered.iter_mut() {
                let line_idx = line.line_number.saturating_sub(viewport_start);
                if line.line_number >= viewport_start {
                    line.decorations.push(dec.clone());
                }
                let _ = line_idx; // used for potential offset calculations
            }
        }
        // Sort each line's decorations by priority
        for line in rendered.iter_mut() {
            line.decorations.sort_by_key(|d| d.kind.priority());
        }
    }

    /// Produce cursor display info relative to the viewport.
    pub fn render_cursor(&self, cursor_line: u32, cursor_col: u32) -> CursorDisplay {
        CursorDisplay {
            line: cursor_line,
            column: cursor_col,
            is_visible: self.viewport.is_visible(cursor_line),
            style: self.cursor_style,
        }
    }

    /// Whether the current line should be highlighted.
    pub fn render_current_line_highlight(&self, cursor_line: u32) -> Option<u32> {
        if self.show_current_line_highlight && self.viewport.is_visible(cursor_line) {
            Some(cursor_line)
        } else {
            None
        }
    }
}

impl Default for EditorRenderer {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn viewport_state_new() {
        let vp = ViewportState::new(30);
        assert_eq!(vp.height, 30);
        assert_eq!(vp.first_visible_line, 1);
        assert_eq!(vp.last_visible_line, 30);
    }

    #[test]
    fn viewport_update() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        assert_eq!(vp.first_visible_line, 5);
        assert_eq!(vp.last_visible_line, 14);
        assert_eq!(vp.scroll_top, 4);
    }

    #[test]
    fn viewport_is_visible() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        assert!(vp.is_visible(5));
        assert!(vp.is_visible(14));
        assert!(!vp.is_visible(4));
        assert!(!vp.is_visible(15));
    }

    #[test]
    fn viewport_clamp_to_end() {
        let mut vp = ViewportState::new(10);
        vp.update(95, 100);
        assert_eq!(vp.first_visible_line, 95);
        assert_eq!(vp.last_visible_line, 100);
        assert_eq!(vp.visible_line_count(), 6);
    }

    #[test]
    fn render_viewport_basic() {
        let renderer = EditorRenderer::new();
        let lines: Vec<&str> = (0..50).map(|_| "hello").collect();
        let rendered = renderer.render_viewport(&lines, 1);
        assert_eq!(rendered.len(), 24);
        assert_eq!(rendered[0].line_number, 1);
        assert!(rendered[0].is_current_line);
        assert!(!rendered[1].is_current_line);
    }

    #[test]
    fn render_cursor_visible() {
        let renderer = EditorRenderer::new();
        let cursor = renderer.render_cursor(5, 10);
        assert!(cursor.is_visible);
        assert_eq!(cursor.line, 5);
        assert_eq!(cursor.column, 10);
        assert_eq!(cursor.style, CursorStyle::Block);
    }

    #[test]
    fn render_cursor_not_visible() {
        let renderer = EditorRenderer::new();
        let cursor = renderer.render_cursor(30, 1);
        assert!(!cursor.is_visible);
    }

    #[test]
    fn current_line_highlight() {
        let renderer = EditorRenderer::new();
        assert_eq!(renderer.render_current_line_highlight(5), Some(5));
        assert_eq!(renderer.render_current_line_highlight(30), None);
    }

    #[test]
    fn decoration_priority_order() {
        assert!(DecorationKind::Selection.priority() > DecorationKind::Error.priority());
        assert!(DecorationKind::Error.priority() > DecorationKind::SearchMatch.priority());
        assert!(DecorationKind::SearchMatch.priority() > DecorationKind::WordHighlight.priority());
    }
}
