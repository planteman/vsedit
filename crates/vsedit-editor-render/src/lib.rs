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

// ---------------------------------------------------------------------------
// Additional rendering utilities
// ---------------------------------------------------------------------------

impl RenderedEditorLine {
    /// Create a new rendered line with no decorations.
    pub fn new(line_number: u32, content: impl Into<String>) -> Self {
        Self {
            line_number,
            content: content.into(),
            decorations: Vec::new(),
            is_current_line: false,
            is_wrapped: false,
        }
    }

    /// Return decorations of a specific kind.
    pub fn decorations_of_kind(&self, kind: DecorationKind) -> Vec<&LineDecoration> {
        self.decorations.iter().filter(|d| d.kind == kind).collect()
    }

    /// Whether the line has any diagnostic decorations (error/warning/info/hint).
    pub fn has_diagnostics(&self) -> bool {
        self.decorations.iter().any(|d| matches!(
            d.kind,
            DecorationKind::Error | DecorationKind::Warning |
            DecorationKind::Info | DecorationKind::Hint
        ))
    }

    /// Total number of columns covered by all decorations (sum of spans).
    pub fn total_decoration_span(&self) -> u32 {
        self.decorations.iter().map(|d| d.end_col.saturating_sub(d.start_col)).sum()
    }

    /// Check if any decoration overlaps the given column range [start, end).
    pub fn has_decoration_in_range(&self, start: u32, end: u32) -> bool {
        self.decorations.iter().any(|d| d.start_col < end && d.end_col > start)
    }

    /// Visible content length (character count).
    pub fn content_len(&self) -> usize {
        self.content.len()
    }
}

impl PartialEq for RenderedEditorLine {
    fn eq(&self, other: &Self) -> bool {
        self.line_number == other.line_number
            && self.content == other.content
            && self.is_current_line == other.is_current_line
            && self.is_wrapped == other.is_wrapped
    }
}

impl LineDecoration {
    /// Create a new decoration spanning [start_col, end_col).
    pub fn new(start_col: u32, end_col: u32, kind: DecorationKind) -> Self {
        Self { start_col, end_col, kind }
    }

    /// The width of the decorated span.
    pub fn span(&self) -> u32 {
        self.end_col.saturating_sub(self.start_col)
    }

    /// Whether two decorations overlap.
    pub fn overlaps(&self, other: &LineDecoration) -> bool {
        self.start_col < other.end_col && self.end_col > other.start_col
    }

    /// Whether this decoration is a diagnostic type.
    pub fn is_diagnostic(&self) -> bool {
        matches!(
            self.kind,
            DecorationKind::Error | DecorationKind::Warning |
            DecorationKind::Info | DecorationKind::Hint
        )
    }

    /// Whether this decoration is a git gutter type.
    pub fn is_git_gutter(&self) -> bool {
        matches!(
            self.kind,
            DecorationKind::GitGutterAdd | DecorationKind::GitGutterModify |
            DecorationKind::GitGutterDelete
        )
    }
}

impl PartialEq for LineDecoration {
    fn eq(&self, other: &Self) -> bool {
        self.start_col == other.start_col
            && self.end_col == other.end_col
            && self.kind == other.kind
    }
}

impl ViewportState {
    /// Scroll down by the given number of lines (clamped to total).
    pub fn scroll_down(&mut self, lines: u32) {
        let new_first = (self.first_visible_line + lines).min(
            self.total_lines.saturating_sub(self.height).saturating_add(1).max(1),
        );
        self.update(new_first, self.total_lines);
    }

    /// Scroll up by the given number of lines (clamped to 1).
    pub fn scroll_up(&mut self, lines: u32) {
        let new_first = self.first_visible_line.saturating_sub(lines).max(1);
        self.update(new_first, self.total_lines);
    }

    /// Scroll to ensure the given line is visible. Returns true if scrolling occurred.
    pub fn ensure_visible(&mut self, line: u32) -> bool {
        if self.is_visible(line) {
            return false;
        }
        if line < self.first_visible_line {
            self.update(line, self.total_lines);
        } else {
            let new_first = line.saturating_sub(self.height - 1);
            self.update(new_first.max(1), self.total_lines);
        }
        true
    }

    /// Scroll percentage (0.0 to 1.0).
    pub fn scroll_fraction(&self) -> f64 {
        if self.total_lines <= self.height {
            0.0
        } else {
            (self.first_visible_line.saturating_sub(1)) as f64
                / (self.total_lines.saturating_sub(self.height)) as f64
        }
    }

    /// Page down: scroll by one full viewport height.
    pub fn page_down(&mut self) {
        self.scroll_down(self.height);
    }

    /// Page up: scroll by one full viewport height.
    pub fn page_up(&mut self) {
        self.scroll_up(self.height);
    }

    /// Jump to the very top of the document.
    pub fn go_to_top(&mut self) {
        self.update(1, self.total_lines);
    }

    /// Jump to the very bottom of the document.
    pub fn go_to_bottom(&mut self) {
        let first = self.total_lines.saturating_sub(self.height).saturating_add(1).max(1);
        self.update(first, self.total_lines);
    }
}

impl std::fmt::Display for ViewportState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Viewport[{}-{} of {} (h={})]",
            self.first_visible_line, self.last_visible_line,
            self.total_lines, self.height
        )
    }
}

impl std::fmt::Display for CursorStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CursorStyle::Block => write!(f, "Block"),
            CursorStyle::Line => write!(f, "Line"),
            CursorStyle::Underline => write!(f, "Underline"),
        }
    }
}

impl std::fmt::Display for DecorationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl EditorRenderer {
    /// Set cursor style and return self for chaining.
    pub fn with_cursor_style(mut self, style: CursorStyle) -> Self {
        self.cursor_style = style;
        self
    }

    /// Set viewport height and return self for chaining.
    pub fn with_height(mut self, height: u32) -> Self {
        self.viewport = ViewportState::new(height);
        self
    }

    /// Scroll to ensure the cursor line is visible, returning whether the viewport changed.
    pub fn ensure_cursor_visible(&mut self, cursor_line: u32) -> bool {
        self.viewport.ensure_visible(cursor_line)
    }

    /// Build a gutter string for the given line number (right-aligned).
    pub fn format_line_number(&self, line_number: u32) -> String {
        format!("{:>width$} ", line_number, width = self.line_number_width as usize)
    }

    /// Merge overlapping decorations of the same kind on a single line.
    pub fn merge_same_kind(decorations: &mut Vec<LineDecoration>) {
        decorations.sort_by_key(|d| (d.kind as u8, d.start_col));
        let mut merged: Vec<LineDecoration> = Vec::new();
        for dec in decorations.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.kind == dec.kind && dec.start_col <= last.end_col {
                    last.end_col = last.end_col.max(dec.end_col);
                    continue;
                }
            }
            merged.push(dec);
        }
        *decorations = merged;
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

    #[test]
    fn rendered_line_new_defaults() {
        let line = RenderedEditorLine::new(1, "hello");
        assert_eq!(line.line_number, 1);
        assert_eq!(line.content, "hello");
        assert!(!line.is_current_line);
        assert!(!line.is_wrapped);
        assert!(line.decorations.is_empty());
    }

    #[test]
    fn rendered_line_has_diagnostics() {
        let mut line = RenderedEditorLine::new(1, "x");
        assert!(!line.has_diagnostics());
        line.decorations.push(LineDecoration::new(0, 5, DecorationKind::Error));
        assert!(line.has_diagnostics());
    }

    #[test]
    fn rendered_line_decoration_span() {
        let mut line = RenderedEditorLine::new(1, "hello world");
        line.decorations.push(LineDecoration::new(0, 5, DecorationKind::Selection));
        line.decorations.push(LineDecoration::new(6, 11, DecorationKind::SearchMatch));
        assert_eq!(line.total_decoration_span(), 10);
    }

    #[test]
    fn rendered_line_has_decoration_in_range() {
        let mut line = RenderedEditorLine::new(1, "abcdef");
        line.decorations.push(LineDecoration::new(2, 4, DecorationKind::Error));
        assert!(line.has_decoration_in_range(3, 5));
        assert!(!line.has_decoration_in_range(4, 6));
    }

    #[test]
    fn decoration_overlap_check() {
        let a = LineDecoration::new(0, 5, DecorationKind::Selection);
        let b = LineDecoration::new(3, 8, DecorationKind::Error);
        let c = LineDecoration::new(5, 10, DecorationKind::Warning);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn decoration_is_diagnostic() {
        assert!(LineDecoration::new(0, 1, DecorationKind::Error).is_diagnostic());
        assert!(LineDecoration::new(0, 1, DecorationKind::Warning).is_diagnostic());
        assert!(LineDecoration::new(0, 1, DecorationKind::Info).is_diagnostic());
        assert!(LineDecoration::new(0, 1, DecorationKind::Hint).is_diagnostic());
        assert!(!LineDecoration::new(0, 1, DecorationKind::Selection).is_diagnostic());
    }

    #[test]
    fn decoration_is_git_gutter() {
        assert!(LineDecoration::new(0, 1, DecorationKind::GitGutterAdd).is_git_gutter());
        assert!(LineDecoration::new(0, 1, DecorationKind::GitGutterModify).is_git_gutter());
        assert!(LineDecoration::new(0, 1, DecorationKind::GitGutterDelete).is_git_gutter());
        assert!(!LineDecoration::new(0, 1, DecorationKind::Error).is_git_gutter());
    }

    #[test]
    fn viewport_scroll_down_up() {
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        vp.scroll_down(5);
        assert_eq!(vp.first_visible_line, 6);
        vp.scroll_up(3);
        assert_eq!(vp.first_visible_line, 3);
        vp.scroll_up(100);
        assert_eq!(vp.first_visible_line, 1);
    }

    #[test]
    fn viewport_ensure_visible() {
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        assert!(!vp.ensure_visible(5));
        assert!(vp.ensure_visible(20));
        assert!(vp.is_visible(20));
    }

    #[test]
    fn viewport_scroll_fraction() {
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        assert!((vp.scroll_fraction() - 0.0).abs() < 0.01);
        vp.go_to_bottom();
        assert!((vp.scroll_fraction() - 1.0).abs() < 0.01);
    }

    #[test]
    fn viewport_page_down_up() {
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        vp.page_down();
        assert_eq!(vp.first_visible_line, 11);
        vp.page_up();
        assert_eq!(vp.first_visible_line, 1);
    }

    #[test]
    fn viewport_go_to_top_bottom() {
        let mut vp = ViewportState::new(10);
        vp.update(50, 100);
        vp.go_to_top();
        assert_eq!(vp.first_visible_line, 1);
        vp.go_to_bottom();
        assert_eq!(vp.first_visible_line, 91);
        assert_eq!(vp.last_visible_line, 100);
    }

    #[test]
    fn viewport_display() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        let s = format!("{}", vp);
        assert!(s.contains("5-14"));
        assert!(s.contains("100"));
    }

    #[test]
    fn cursor_style_display() {
        assert_eq!(CursorStyle::Block.to_string(), "Block");
        assert_eq!(CursorStyle::Line.to_string(), "Line");
        assert_eq!(CursorStyle::Underline.to_string(), "Underline");
    }

    #[test]
    fn editor_renderer_builder_methods() {
        let r = EditorRenderer::new()
            .with_cursor_style(CursorStyle::Line)
            .with_height(40);
        assert_eq!(r.cursor_style, CursorStyle::Line);
        assert_eq!(r.viewport.height, 40);
    }

    #[test]
    fn format_line_number_padding() {
        let r = EditorRenderer::new();
        let s = r.format_line_number(1);
        assert!(s.contains("1"));
        assert!(s.len() >= 5);
    }

    #[test]
    fn merge_same_kind_decorations() {
        let mut decs = vec![
            LineDecoration::new(0, 5, DecorationKind::Selection),
            LineDecoration::new(3, 8, DecorationKind::Selection),
            LineDecoration::new(10, 15, DecorationKind::Selection),
        ];
        EditorRenderer::merge_same_kind(&mut decs);
        assert_eq!(decs.len(), 2);
        assert_eq!(decs[0].start_col, 0);
        assert_eq!(decs[0].end_col, 8);
        assert_eq!(decs[1].start_col, 10);
    }

    #[test]
    fn ensure_cursor_visible_scrolls() {
        let mut r = EditorRenderer::new().with_height(10);
        r.viewport.update(1, 100);
        assert!(!r.ensure_cursor_visible(5));
        assert!(r.ensure_cursor_visible(50));
        assert!(r.viewport.is_visible(50));
    }

    #[test]
    fn decoration_kind_display() {
        assert_eq!(DecorationKind::Selection.to_string(), "Selection");
        assert_eq!(DecorationKind::Error.to_string(), "Error");
    }

    #[test]
    fn rendered_line_content_len() {
        let line = RenderedEditorLine::new(1, "hello");
        assert_eq!(line.content_len(), 5);
    }

    #[test]
    fn rendered_line_partial_eq() {
        let a = RenderedEditorLine::new(1, "hello");
        let b = RenderedEditorLine::new(1, "hello");
        let c = RenderedEditorLine::new(2, "hello");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
