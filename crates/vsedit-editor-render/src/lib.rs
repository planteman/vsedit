//! Terminal editor line rendering.
//!
//! Provides viewport-aware rendering of editor lines with decoration merging
//! for selections, search highlights, diagnostics, and other visual markers.

use std::fmt;
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
    CodeLens,
    DocumentLink,
    GutterBreakpoint,
    GutterError,
    GutterWarning,
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
            Self::GutterBreakpoint | Self::GutterError | Self::GutterWarning => 5,
            Self::FoldedRegion => 6,
            Self::InlayHint => 7,
            Self::CodeLens => 8,
            Self::DocumentLink => 9,
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

    /// Whether this decoration is a gutter indicator (breakpoint, error, warning, or git).
    pub fn is_gutter(&self) -> bool {
        matches!(
            self.kind,
            DecorationKind::GutterBreakpoint
                | DecorationKind::GutterError
                | DecorationKind::GutterWarning
                | DecorationKind::GitGutterAdd
                | DecorationKind::GitGutterModify
                | DecorationKind::GitGutterDelete
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

/// Resolve which decoration should win when two overlap at the same position.
/// Returns the one with higher priority.
pub fn resolve_decoration_priority<'a>(a: &'a LineDecoration, b: &'a LineDecoration) -> &'a LineDecoration {
    if a.kind.priority() >= b.kind.priority() { a } else { b }
}

/// Clamp a visible range to the viewport boundaries.
pub fn clamp_visible_range(start: u32, end: u32, viewport: &ViewportState) -> (u32, u32) {
    let clamped_start = start.max(viewport.first_visible_line);
    let clamped_end = end.min(viewport.last_visible_line);
    if clamped_start > clamped_end {
        (clamped_start, clamped_start)
    } else {
        (clamped_start, clamped_end)
    }
}

/// Format a line number with a given width and optional relative mode.
pub fn format_line_number_relative(line: u32, cursor_line: u32, width: usize, relative: bool) -> String {
    if relative {
        if line == cursor_line {
            format!("{:>w$} ", line, w = width)
        } else {
            let diff = (line as i64 - cursor_line as i64).unsigned_abs();
            format!("{:>w$} ", diff, w = width)
        }
    } else {
        format!("{:>w$} ", line, w = width)
    }
}

/// Tracks whether a render cache should be invalidated.
#[derive(Debug, Clone)]
pub struct RenderCacheState {
    pub last_viewport_first: u32,
    pub last_viewport_height: u32,
    pub last_cursor_line: u32,
    pub last_total_lines: u32,
    pub dirty: bool,
}

impl RenderCacheState {
    pub fn new() -> Self {
        Self {
            last_viewport_first: 0,
            last_viewport_height: 0,
            last_cursor_line: 0,
            last_total_lines: 0,
            dirty: true,
        }
    }

    /// Check if the cache needs invalidation given the current state.
    pub fn needs_invalidation(&self, viewport: &ViewportState, cursor_line: u32) -> bool {
        self.dirty
            || self.last_viewport_first != viewport.first_visible_line
            || self.last_viewport_height != viewport.height
            || self.last_cursor_line != cursor_line
            || self.last_total_lines != viewport.total_lines
    }

    /// Update the cache state to reflect the current viewport and cursor.
    pub fn update(&mut self, viewport: &ViewportState, cursor_line: u32) {
        self.last_viewport_first = viewport.first_visible_line;
        self.last_viewport_height = viewport.height;
        self.last_cursor_line = cursor_line;
        self.last_total_lines = viewport.total_lines;
        self.dirty = false;
    }

    /// Mark the cache as dirty so the next render will regenerate.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
}

impl Default for RenderCacheState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Word wrap
// ---------------------------------------------------------------------------

/// Word wrap mode for the editor renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrapMode {
    /// No wrapping — lines extend beyond the viewport.
    Off,
    /// Wrap at a fixed column (character-based).
    On(usize),
    /// Wrap at word boundaries, falling back to character wrap for long words.
    WordBoundary(usize),
    /// Wrap at a specific column (alias for `On`).
    Bounded(usize),
}

/// A wrapped line that tracks which original document line it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedLine {
    /// The 0-based index of the original document line.
    pub original_line: usize,
    /// The content of this wrapped segment.
    pub content: String,
    /// Whether this is a continuation of the previous line (not the first
    /// segment).
    pub is_continuation: bool,
}

/// Wrap a single line according to the given mode and maximum width.
pub fn wrap_line(line: &str, max_width: usize, mode: WordWrapMode) -> Vec<String> {
    match mode {
        WordWrapMode::Off => vec![line.to_string()],
        WordWrapMode::On(w) | WordWrapMode::Bounded(w) => {
            let width = if w == 0 { max_width } else { w };
            char_wrap(line, width)
        }
        WordWrapMode::WordBoundary(w) => {
            let width = if w == 0 { max_width } else { w };
            word_boundary_wrap(line, width)
        }
    }
}

/// Character-based wrapping at `width` columns.
fn char_wrap(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    if line.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= width {
        return vec![line.to_string()];
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        result.push(chars[start..end].iter().collect());
        start = end;
    }
    result
}

/// Word-boundary-aware wrapping at `width` columns.
fn word_boundary_wrap(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    if line.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= width {
        return vec![line.to_string()];
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        if start + width >= chars.len() {
            result.push(chars[start..].iter().collect());
            break;
        }
        // Look for the last space within [start..start+width].
        let segment = &chars[start..start + width];
        if let Some(space_pos) = segment.iter().rposition(|&c| c == ' ') {
            // Wrap at the space — include the space in this line.
            result.push(chars[start..start + space_pos + 1].iter().collect());
            start += space_pos + 1;
        } else {
            // No space found — fall back to hard character wrap.
            result.push(segment.iter().collect());
            start += width;
        }
    }
    result
}

/// Wrap an entire document, returning a flat list of [`WrappedLine`]s.
pub fn wrap_document(lines: &[&str], width: usize, mode: WordWrapMode) -> Vec<WrappedLine> {
    let mut result = Vec::new();
    for (idx, &line) in lines.iter().enumerate() {
        let segments = wrap_line(line, width, mode);
        for (seg_idx, seg) in segments.into_iter().enumerate() {
            result.push(WrappedLine {
                original_line: idx,
                content: seg,
                is_continuation: seg_idx > 0,
            });
        }
    }
    result
}

// ---------------------------------------------------------------------------
// RenderedEditorLine extensions
// ---------------------------------------------------------------------------

impl RenderedEditorLine {
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    pub fn has_decorations(&self) -> bool {
        !self.decorations.is_empty()
    }

    pub fn decoration_count(&self) -> usize {
        self.decorations.len()
    }
}

// ---------------------------------------------------------------------------
// LineDecoration extensions
// ---------------------------------------------------------------------------

impl LineDecoration {
    pub fn contains_column(&self, col: u32) -> bool {
        col >= self.start_col && col < self.end_col
    }

    pub fn length(&self) -> u32 {
        self.end_col.saturating_sub(self.start_col)
    }
}

// ---------------------------------------------------------------------------
// DecorationKind extensions
// ---------------------------------------------------------------------------

impl DecorationKind {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error | Self::GutterError)
    }

    pub fn is_warning(&self) -> bool {
        matches!(self, Self::Warning | Self::GutterWarning)
    }

    pub fn is_info(&self) -> bool {
        matches!(self, Self::Info)
    }

    pub fn severity_rank(&self) -> Option<u8> {
        match self {
            Self::Hint => Some(1),
            Self::Info => Some(2),
            Self::Warning | Self::GutterWarning => Some(3),
            Self::Error | Self::GutterError => Some(4),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ViewportState extensions
// ---------------------------------------------------------------------------

impl ViewportState {
    pub fn center_line(&self) -> u32 {
        self.first_visible_line + self.visible_line_count() / 2
    }

    pub fn scroll_percentage(&self, total: u32) -> f64 {
        if total == 0 {
            return 0.0;
        }
        (self.last_visible_line as f64 / total as f64) * 100.0
    }
}

// ---------------------------------------------------------------------------
// CursorDisplay extensions
// ---------------------------------------------------------------------------

impl fmt::Display for CursorDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Cursor({}:{} {} {})",
            self.line,
            self.column,
            self.style,
            if self.is_visible { "visible" } else { "hidden" }
        )
    }
}

// ---------------------------------------------------------------------------
// CursorStyle extensions
// ---------------------------------------------------------------------------

impl CursorStyle {
    pub fn is_block(&self) -> bool {
        matches!(self, Self::Block)
    }

    pub fn is_line(&self) -> bool {
        matches!(self, Self::Line)
    }
}

// ---------------------------------------------------------------------------
// WordWrapMode extensions
// ---------------------------------------------------------------------------

impl fmt::Display for WordWrapMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::On(col) => write!(f, "On({})", col),
            Self::WordBoundary(col) => write!(f, "WordBoundary({})", col),
            Self::Bounded(col) => write!(f, "Bounded({})", col),
        }
    }
}

impl WordWrapMode {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn effective_column(&self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::On(c) | Self::WordBoundary(c) | Self::Bounded(c) => Some(*c),
        }
    }
}

// ---------------------------------------------------------------------------
// WrappedLine extensions
// ---------------------------------------------------------------------------

impl WrappedLine {
    pub fn visual_line_count(lines: &[WrappedLine]) -> usize {
        lines.len()
    }

    pub fn is_wrapped(&self) -> bool {
        self.is_continuation
    }

    pub fn original_length(&self) -> usize {
        self.content.len()
    }
}

// ---------------------------------------------------------------------------
// RenderCacheState extensions
// ---------------------------------------------------------------------------

impl RenderCacheState {
    pub fn is_valid(&self) -> bool {
        !self.dirty
    }
}

impl fmt::Display for RenderCacheState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RenderCache(line={} h={} cursor={} total={} {})",
            self.last_viewport_first,
            self.last_viewport_height,
            self.last_cursor_line,
            self.last_total_lines,
            if self.dirty { "dirty" } else { "clean" }
        )
    }
}

// ---------------------------------------------------------------------------
// EditorRenderer extensions
// ---------------------------------------------------------------------------

impl EditorRenderer {
    pub fn total_rendered_lines(&self) -> u32 {
        self.viewport.visible_line_count()
    }
}

// ---------------------------------------------------------------------------
// Line token layout computation
// ---------------------------------------------------------------------------

/// A positioned token ready for rendering, with pre-computed column offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutToken {
    /// Start column (0-based, in display cells).
    pub start_col: u32,
    /// End column (exclusive, in display cells).
    pub end_col: u32,
    /// The text content of this token.
    pub text: String,
}

/// Default tab stop interval.
const DEFAULT_TAB_WIDTH: u32 = 4;

/// Compute the display width of a single character at a given column,
/// expanding tabs to the next tab stop.
pub fn char_display_width(ch: char, current_col: u32, tab_width: u32) -> u32 {
    match ch {
        '\t' => {
            let tw = tab_width.max(1);
            tw - (current_col % tw)
        }
        _ => 1,
    }
}

/// Compute the laid-out token positions for a line, honoring tab stops.
/// Each token is a contiguous run of non-tab characters or a single tab.
pub fn layout_line_tokens(line: &str, tab_width: u32) -> Vec<LayoutToken> {
    let mut tokens = Vec::new();
    let mut col: u32 = 0;
    let mut current_text = String::new();
    let mut token_start = col;

    for ch in line.chars() {
        if ch == '\t' {
            // Flush any accumulated text token.
            if !current_text.is_empty() {
                tokens.push(LayoutToken {
                    start_col: token_start,
                    end_col: col,
                    text: current_text.clone(),
                });
                current_text.clear();
            }
            let width = char_display_width('\t', col, tab_width);
            tokens.push(LayoutToken {
                start_col: col,
                end_col: col + width,
                text: "\t".to_string(),
            });
            col += width;
            token_start = col;
        } else {
            if current_text.is_empty() {
                token_start = col;
            }
            current_text.push(ch);
            col += 1;
        }
    }
    if !current_text.is_empty() {
        tokens.push(LayoutToken {
            start_col: token_start,
            end_col: col,
            text: current_text,
        });
    }
    tokens
}

/// Compute the total display width of a line (expanding tabs).
pub fn line_display_width(line: &str, tab_width: u32) -> u32 {
    let mut col: u32 = 0;
    for ch in line.chars() {
        col += char_display_width(ch, col, tab_width);
    }
    col
}

// ---------------------------------------------------------------------------
// Viewport dirty region tracking
// ---------------------------------------------------------------------------

/// Tracks which lines within the viewport need re-rendering.
#[derive(Debug, Clone)]
pub struct DirtyRegionTracker {
    /// Bitmap of dirty line offsets relative to viewport start.
    dirty_lines: Vec<bool>,
    /// Total viewport height this tracker covers.
    height: usize,
}

impl DirtyRegionTracker {
    /// Create a new tracker for the given viewport height, initially all clean.
    pub fn new(height: usize) -> Self {
        Self {
            dirty_lines: vec![false; height],
            height,
        }
    }

    /// Mark all lines dirty (e.g. after a scroll).
    pub fn mark_all_dirty(&mut self) {
        self.dirty_lines.iter_mut().for_each(|d| *d = true);
    }

    /// Mark all lines clean after a full re-render.
    pub fn clear(&mut self) {
        self.dirty_lines.iter_mut().for_each(|d| *d = false);
    }

    /// Mark a single viewport-relative line offset as dirty.
    pub fn mark_dirty(&mut self, offset: usize) {
        if offset < self.height {
            self.dirty_lines[offset] = true;
        }
    }

    /// Mark a range of viewport-relative line offsets as dirty.
    pub fn mark_range_dirty(&mut self, start: usize, end: usize) {
        let clamped_end = end.min(self.height);
        for i in start..clamped_end {
            self.dirty_lines[i] = true;
        }
    }

    /// Whether the given viewport-relative offset needs re-rendering.
    pub fn is_dirty(&self, offset: usize) -> bool {
        offset < self.height && self.dirty_lines[offset]
    }

    /// Returns the offsets of all dirty lines.
    pub fn dirty_offsets(&self) -> Vec<usize> {
        self.dirty_lines
            .iter()
            .enumerate()
            .filter_map(|(i, &d)| if d { Some(i) } else { None })
            .collect()
    }

    /// Number of dirty lines.
    pub fn dirty_count(&self) -> usize {
        self.dirty_lines.iter().filter(|&&d| d).count()
    }

    /// Whether there are any dirty lines at all.
    pub fn has_dirty(&self) -> bool {
        self.dirty_lines.iter().any(|&d| d)
    }

    /// Resize the tracker for a new viewport height, marking new lines dirty.
    pub fn resize(&mut self, new_height: usize) {
        if new_height > self.height {
            self.dirty_lines.resize(new_height, true);
        } else {
            self.dirty_lines.truncate(new_height);
        }
        self.height = new_height;
    }
}

// ---------------------------------------------------------------------------
// Inline decoration placement
// ---------------------------------------------------------------------------

/// A positioned inline decoration with resolved column offsets after tab
/// expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDecoration {
    /// Display column start (0-based).
    pub display_start: u32,
    /// Display column end (exclusive).
    pub display_end: u32,
    /// The decoration kind.
    pub kind: DecorationKind,
}

/// Map a byte/char-offset–based decoration to display columns, expanding tabs.
pub fn resolve_decoration_columns(
    line: &str,
    decoration: &LineDecoration,
    tab_width: u32,
) -> ResolvedDecoration {
    let mut col: u32 = 0;
    let mut char_idx: u32 = 0;
    let mut display_start = 0u32;
    let mut display_end = 0u32;
    let mut found_start = false;

    for ch in line.chars() {
        if char_idx == decoration.start_col {
            display_start = col;
            found_start = true;
        }
        if char_idx == decoration.end_col {
            display_end = col;
            return ResolvedDecoration {
                display_start,
                display_end,
                kind: decoration.kind,
            };
        }
        col += char_display_width(ch, col, tab_width);
        char_idx += 1;
    }
    // Decoration extends to or past end of line.
    if !found_start {
        display_start = col;
    }
    display_end = col;
    ResolvedDecoration {
        display_start,
        display_end,
        kind: decoration.kind,
    }
}

// ---------------------------------------------------------------------------
// Cursor blink phase state machine
// ---------------------------------------------------------------------------

/// Cursor blink phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlinkPhase {
    /// Cursor is visible.
    Visible,
    /// Cursor is hidden (between blinks).
    Hidden,
    /// Blinking is paused (e.g. user just typed).
    Paused,
}

/// State machine that drives cursor blink timing.
#[derive(Debug, Clone)]
pub struct CursorBlinkState {
    /// Current phase.
    pub phase: BlinkPhase,
    /// Milliseconds elapsed in the current phase.
    pub elapsed_ms: u64,
    /// Duration of the visible phase in milliseconds.
    pub visible_ms: u64,
    /// Duration of the hidden phase in milliseconds.
    pub hidden_ms: u64,
    /// Duration of the pause phase in milliseconds (after a keystroke).
    pub pause_ms: u64,
    /// Whether blinking is enabled at all.
    pub enabled: bool,
}

impl CursorBlinkState {
    /// Create with default blink timings (530ms visible, 530ms hidden, 800ms pause).
    pub fn new() -> Self {
        Self {
            phase: BlinkPhase::Visible,
            elapsed_ms: 0,
            visible_ms: 530,
            hidden_ms: 530,
            pause_ms: 800,
            enabled: true,
        }
    }

    /// Advance the state machine by `delta_ms` milliseconds.
    /// Returns `true` if the phase changed.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.enabled {
            return false;
        }
        self.elapsed_ms += delta_ms;
        let threshold = match self.phase {
            BlinkPhase::Visible => self.visible_ms,
            BlinkPhase::Hidden => self.hidden_ms,
            BlinkPhase::Paused => self.pause_ms,
        };
        if self.elapsed_ms >= threshold {
            self.elapsed_ms = 0;
            self.phase = match self.phase {
                BlinkPhase::Visible => BlinkPhase::Hidden,
                BlinkPhase::Hidden => BlinkPhase::Visible,
                BlinkPhase::Paused => BlinkPhase::Visible,
            };
            true
        } else {
            false
        }
    }

    /// Reset to paused-visible (call after user input).
    pub fn reset_on_input(&mut self) {
        self.phase = BlinkPhase::Paused;
        self.elapsed_ms = 0;
    }

    /// Whether the cursor should currently be drawn.
    pub fn should_draw(&self) -> bool {
        !self.enabled || self.phase != BlinkPhase::Hidden
    }
}

impl Default for CursorBlinkState {
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

    #[test]
    fn test_resolve_decoration_priority() {
        let error = LineDecoration::new(0, 5, DecorationKind::Error);
        let selection = LineDecoration::new(0, 5, DecorationKind::Selection);
        let winner = resolve_decoration_priority(&error, &selection);
        assert_eq!(winner.kind, DecorationKind::Selection);
    }

    #[test]
    fn test_resolve_decoration_priority_same_kind() {
        let a = LineDecoration::new(0, 5, DecorationKind::Error);
        let b = LineDecoration::new(3, 8, DecorationKind::Error);
        let winner = resolve_decoration_priority(&a, &b);
        assert_eq!(winner.start_col, 0);
    }

    #[test]
    fn test_clamp_visible_range_within() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        let (s, e) = clamp_visible_range(6, 12, &vp);
        assert_eq!(s, 6);
        assert_eq!(e, 12);
    }

    #[test]
    fn test_clamp_visible_range_outside() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        let (s, e) = clamp_visible_range(1, 3, &vp);
        assert_eq!(s, 5);
        assert_eq!(e, 5);
    }

    #[test]
    fn test_clamp_visible_range_partial() {
        let mut vp = ViewportState::new(10);
        vp.update(5, 100);
        let (s, e) = clamp_visible_range(3, 8, &vp);
        assert_eq!(s, 5);
        assert_eq!(e, 8);
    }

    #[test]
    fn test_format_line_number_absolute() {
        let s = format_line_number_relative(42, 10, 4, false);
        assert!(s.contains("42"));
    }

    #[test]
    fn test_format_line_number_relative_current() {
        let s = format_line_number_relative(10, 10, 4, true);
        assert!(s.contains("10"));
    }

    #[test]
    fn test_format_line_number_relative_offset() {
        let s = format_line_number_relative(13, 10, 4, true);
        assert!(s.contains("3"));
    }

    #[test]
    fn test_render_cache_state_initial_dirty() {
        let cache = RenderCacheState::new();
        assert!(cache.dirty);
        let vp = ViewportState::new(10);
        assert!(cache.needs_invalidation(&vp, 1));
    }

    #[test]
    fn test_render_cache_state_update_clears_dirty() {
        let mut cache = RenderCacheState::new();
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        cache.update(&vp, 1);
        assert!(!cache.dirty);
        assert!(!cache.needs_invalidation(&vp, 1));
    }

    #[test]
    fn test_render_cache_state_detects_viewport_change() {
        let mut cache = RenderCacheState::new();
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        cache.update(&vp, 1);
        vp.scroll_down(5);
        assert!(cache.needs_invalidation(&vp, 1));
    }

    #[test]
    fn test_render_cache_invalidate() {
        let mut cache = RenderCacheState::new();
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        cache.update(&vp, 1);
        assert!(!cache.dirty);
        cache.invalidate();
        assert!(cache.dirty);
        assert!(cache.needs_invalidation(&vp, 1));
    }

    // -- new decoration kind tests ------------------------------------------

    #[test]
    fn codelens_decoration_priority() {
        assert_eq!(DecorationKind::CodeLens.priority(), 8);
    }

    #[test]
    fn document_link_decoration_priority() {
        assert_eq!(DecorationKind::DocumentLink.priority(), 9);
    }

    #[test]
    fn gutter_breakpoint_priority() {
        assert_eq!(DecorationKind::GutterBreakpoint.priority(), 5);
        assert_eq!(DecorationKind::GutterError.priority(), 5);
        assert_eq!(DecorationKind::GutterWarning.priority(), 5);
    }

    #[test]
    fn is_gutter_returns_true_for_gutter_kinds() {
        assert!(LineDecoration::new(0, 1, DecorationKind::GutterBreakpoint).is_gutter());
        assert!(LineDecoration::new(0, 1, DecorationKind::GutterError).is_gutter());
        assert!(LineDecoration::new(0, 1, DecorationKind::GutterWarning).is_gutter());
        assert!(LineDecoration::new(0, 1, DecorationKind::GitGutterAdd).is_gutter());
        assert!(!LineDecoration::new(0, 1, DecorationKind::Error).is_gutter());
        assert!(!LineDecoration::new(0, 1, DecorationKind::CodeLens).is_gutter());
    }

    // -- word wrap tests ----------------------------------------------------

    #[test]
    fn wrap_off_returns_original() {
        let result = wrap_line("hello world", 5, WordWrapMode::Off);
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn wrap_no_wrap_needed() {
        let result = wrap_line("hello", 10, WordWrapMode::On(10));
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn wrap_on_simple() {
        let result = wrap_line("abcdefghij", 5, WordWrapMode::On(5));
        assert_eq!(result, vec!["abcde", "fghij"]);
    }

    #[test]
    fn wrap_on_uneven() {
        let result = wrap_line("abcdefg", 3, WordWrapMode::On(3));
        assert_eq!(result, vec!["abc", "def", "g"]);
    }

    #[test]
    fn wrap_bounded_alias() {
        let result = wrap_line("abcdefgh", 4, WordWrapMode::Bounded(4));
        assert_eq!(result, vec!["abcd", "efgh"]);
    }

    #[test]
    fn wrap_empty_line() {
        let result = wrap_line("", 10, WordWrapMode::On(10));
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn wrap_word_boundary_simple() {
        let result = wrap_line("hello world foo", 12, WordWrapMode::WordBoundary(12));
        assert_eq!(result, vec!["hello world ", "foo"]);
    }

    #[test]
    fn wrap_word_boundary_no_space() {
        let result = wrap_line("abcdefghij", 5, WordWrapMode::WordBoundary(5));
        assert_eq!(result, vec!["abcde", "fghij"]);
    }

    #[test]
    fn wrap_word_boundary_exact_fit() {
        let result = wrap_line("hello", 5, WordWrapMode::WordBoundary(5));
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn wrap_word_boundary_multiple_segments() {
        let result = wrap_line("the quick brown fox jumps over", 10, WordWrapMode::WordBoundary(10));
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "the quick ");
        assert_eq!(result[1], "brown fox ");
        assert_eq!(result[2], "jumps over");
    }

    #[test]
    fn wrap_unicode_chars() {
        let result = wrap_line("héllo wörld", 6, WordWrapMode::On(6));
        assert_eq!(result, vec!["héllo ", "wörld"]);
    }

    #[test]
    fn wrap_unicode_emoji() {
        let result = wrap_line("a😀b😀c", 3, WordWrapMode::On(3));
        assert_eq!(result, vec!["a😀b", "😀c"]);
    }

    #[test]
    fn wrap_single_char_width() {
        let result = wrap_line("abc", 1, WordWrapMode::On(1));
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn wrap_very_long_word_boundary() {
        let result = wrap_line("superlongword short", 5, WordWrapMode::WordBoundary(5));
        assert_eq!(result, vec!["super", "longw", "ord ", "short"]);
    }

    #[test]
    fn wrap_document_basic() {
        let lines = vec!["hello world", "foo"];
        let result = wrap_document(&lines, 5, WordWrapMode::On(5));
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].original_line, 0);
        assert_eq!(result[0].content, "hello");
        assert!(!result[0].is_continuation);
        assert_eq!(result[1].original_line, 0);
        assert_eq!(result[1].content, " worl");
        assert!(result[1].is_continuation);
        assert_eq!(result[2].original_line, 0);
        assert_eq!(result[2].content, "d");
        assert!(result[2].is_continuation);
        assert_eq!(result[3].original_line, 1);
        assert_eq!(result[3].content, "foo");
        assert!(!result[3].is_continuation);
    }

    #[test]
    fn wrap_document_no_wrap() {
        let lines = vec!["abc", "def"];
        let result = wrap_document(&lines, 10, WordWrapMode::On(10));
        assert_eq!(result.len(), 2);
        assert!(!result[0].is_continuation);
        assert!(!result[1].is_continuation);
    }

    #[test]
    fn wrap_document_empty_lines() {
        let lines = vec!["", "hello", ""];
        let result = wrap_document(&lines, 10, WordWrapMode::On(10));
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "");
        assert_eq!(result[0].original_line, 0);
        assert_eq!(result[2].content, "");
        assert_eq!(result[2].original_line, 2);
    }

    #[test]
    fn wrap_document_off_mode() {
        let lines = vec!["a very long line indeed"];
        let result = wrap_document(&lines, 5, WordWrapMode::Off);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "a very long line indeed");
    }

    // -- extension tests ----------------------------------------------------

    #[test]
    fn rendered_line_is_empty_and_char_count() {
        let empty = RenderedEditorLine::new(1, "");
        assert!(empty.is_empty());
        assert_eq!(empty.char_count(), 0);

        let line = RenderedEditorLine::new(2, "héllo");
        assert!(!line.is_empty());
        assert_eq!(line.char_count(), 5);
    }

    #[test]
    fn rendered_line_has_decorations_and_count() {
        let mut line = RenderedEditorLine::new(1, "abc");
        assert!(!line.has_decorations());
        assert_eq!(line.decoration_count(), 0);

        line.decorations.push(LineDecoration::new(0, 2, DecorationKind::Error));
        line.decorations.push(LineDecoration::new(2, 3, DecorationKind::Warning));
        assert!(line.has_decorations());
        assert_eq!(line.decoration_count(), 2);
    }

    #[test]
    fn line_decoration_contains_column_and_length() {
        let dec = LineDecoration::new(3, 7, DecorationKind::Selection);
        assert!(!dec.contains_column(2));
        assert!(dec.contains_column(3));
        assert!(dec.contains_column(6));
        assert!(!dec.contains_column(7));
        assert_eq!(dec.length(), 4);
    }

    #[test]
    fn decoration_kind_severity_helpers() {
        assert!(DecorationKind::Error.is_error());
        assert!(DecorationKind::GutterError.is_error());
        assert!(!DecorationKind::Warning.is_error());

        assert!(DecorationKind::Warning.is_warning());
        assert!(DecorationKind::GutterWarning.is_warning());
        assert!(!DecorationKind::Error.is_warning());

        assert!(DecorationKind::Info.is_info());
        assert!(!DecorationKind::Hint.is_info());

        assert_eq!(DecorationKind::Error.severity_rank(), Some(4));
        assert_eq!(DecorationKind::Warning.severity_rank(), Some(3));
        assert_eq!(DecorationKind::Info.severity_rank(), Some(2));
        assert_eq!(DecorationKind::Hint.severity_rank(), Some(1));
        assert_eq!(DecorationKind::Selection.severity_rank(), None);
    }

    #[test]
    fn viewport_center_line_and_scroll_percentage() {
        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        assert_eq!(vp.center_line(), 6);
        let pct = vp.scroll_percentage(100);
        assert!((pct - 10.0).abs() < 0.01);

        let empty_pct = vp.scroll_percentage(0);
        assert!((empty_pct - 0.0).abs() < 0.01);
    }

    #[test]
    fn cursor_and_style_extensions() {
        assert!(CursorStyle::Block.is_block());
        assert!(!CursorStyle::Block.is_line());
        assert!(CursorStyle::Line.is_line());
        assert!(!CursorStyle::Line.is_block());

        let cursor = CursorDisplay {
            line: 5,
            column: 10,
            is_visible: true,
            style: CursorStyle::Block,
        };
        let s = format!("{}", cursor);
        assert!(s.contains("5:10"));
        assert!(s.contains("visible"));
    }

    #[test]
    fn word_wrap_mode_display_and_helpers() {
        assert_eq!(WordWrapMode::Off.to_string(), "Off");
        assert_eq!(WordWrapMode::On(80).to_string(), "On(80)");
        assert_eq!(WordWrapMode::WordBoundary(120).to_string(), "WordBoundary(120)");
        assert_eq!(WordWrapMode::Bounded(40).to_string(), "Bounded(40)");

        assert!(!WordWrapMode::Off.is_enabled());
        assert!(WordWrapMode::On(80).is_enabled());
        assert!(WordWrapMode::WordBoundary(120).is_enabled());

        assert_eq!(WordWrapMode::Off.effective_column(), None);
        assert_eq!(WordWrapMode::On(80).effective_column(), Some(80));
        assert_eq!(WordWrapMode::Bounded(40).effective_column(), Some(40));
    }

    #[test]
    fn wrapped_line_extensions() {
        let lines = wrap_document(&["hello world"], 5, WordWrapMode::On(5));
        assert_eq!(WrappedLine::visual_line_count(&lines), 3);
        assert!(!lines[0].is_wrapped());
        assert!(lines[1].is_wrapped());
        assert_eq!(lines[0].original_length(), 5);
    }

    #[test]
    fn render_cache_is_valid_and_display() {
        let mut cache = RenderCacheState::new();
        assert!(!cache.is_valid());

        let mut vp = ViewportState::new(10);
        vp.update(1, 100);
        cache.update(&vp, 5);
        assert!(cache.is_valid());

        let s = format!("{}", cache);
        assert!(s.contains("clean"));
        assert!(s.contains("cursor=5"));

        cache.invalidate();
        let s2 = format!("{}", cache);
        assert!(s2.contains("dirty"));
    }

    #[test]
    fn editor_renderer_total_rendered_lines() {
        let mut r = EditorRenderer::new().with_height(10);
        r.viewport.update(1, 100);
        assert_eq!(r.total_rendered_lines(), 10);

        r.viewport.update(95, 100);
        assert_eq!(r.total_rendered_lines(), 6);
    }

    // -- line token layout tests --------------------------------------------

    #[test]
    fn char_display_width_regular_and_tab() {
        assert_eq!(char_display_width('a', 0, 4), 1);
        assert_eq!(char_display_width('\t', 0, 4), 4);
        assert_eq!(char_display_width('\t', 1, 4), 3);
        assert_eq!(char_display_width('\t', 3, 4), 1);
        assert_eq!(char_display_width('\t', 4, 4), 4);
        // Tab width of 8
        assert_eq!(char_display_width('\t', 5, 8), 3);
    }

    #[test]
    fn layout_line_tokens_plain_text() {
        let tokens = layout_line_tokens("hello", 4);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].start_col, 0);
        assert_eq!(tokens[0].end_col, 5);
        assert_eq!(tokens[0].text, "hello");
    }

    #[test]
    fn layout_line_tokens_with_tabs() {
        let tokens = layout_line_tokens("\thi\tthere", 4);
        // \t(0->4) "hi"(4->6) \t(6->8) "there"(8->13)
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].text, "\t");
        assert_eq!(tokens[0].start_col, 0);
        assert_eq!(tokens[0].end_col, 4);
        assert_eq!(tokens[1].text, "hi");
        assert_eq!(tokens[1].start_col, 4);
        assert_eq!(tokens[1].end_col, 6);
        assert_eq!(tokens[2].text, "\t");
        assert_eq!(tokens[2].start_col, 6);
        assert_eq!(tokens[2].end_col, 8);
        assert_eq!(tokens[3].text, "there");
        assert_eq!(tokens[3].start_col, 8);
        assert_eq!(tokens[3].end_col, 13);
    }

    #[test]
    fn line_display_width_with_tabs() {
        assert_eq!(line_display_width("hello", 4), 5);
        assert_eq!(line_display_width("\t", 4), 4);
        assert_eq!(line_display_width("ab\tcd", 4), 6); // a(0) b(1) \t(2->4) c(4) d(5) = 6
        assert_eq!(line_display_width("", 4), 0);
    }

    // -- dirty region tracker tests -----------------------------------------

    #[test]
    fn dirty_tracker_initial_state() {
        let tracker = DirtyRegionTracker::new(10);
        assert!(!tracker.has_dirty());
        assert_eq!(tracker.dirty_count(), 0);
        assert!(tracker.dirty_offsets().is_empty());
    }

    #[test]
    fn dirty_tracker_mark_and_clear() {
        let mut tracker = DirtyRegionTracker::new(5);
        tracker.mark_dirty(2);
        tracker.mark_dirty(4);
        assert!(tracker.is_dirty(2));
        assert!(tracker.is_dirty(4));
        assert!(!tracker.is_dirty(0));
        assert_eq!(tracker.dirty_count(), 2);
        assert_eq!(tracker.dirty_offsets(), vec![2, 4]);

        tracker.clear();
        assert!(!tracker.has_dirty());
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn dirty_tracker_mark_all_and_range() {
        let mut tracker = DirtyRegionTracker::new(5);
        tracker.mark_all_dirty();
        assert_eq!(tracker.dirty_count(), 5);

        tracker.clear();
        tracker.mark_range_dirty(1, 3);
        assert!(!tracker.is_dirty(0));
        assert!(tracker.is_dirty(1));
        assert!(tracker.is_dirty(2));
        assert!(!tracker.is_dirty(3));
        assert_eq!(tracker.dirty_count(), 2);
    }

    #[test]
    fn dirty_tracker_resize() {
        let mut tracker = DirtyRegionTracker::new(3);
        tracker.mark_dirty(0);
        // Grow: new lines should be dirty
        tracker.resize(5);
        assert!(tracker.is_dirty(0));
        assert!(tracker.is_dirty(3));
        assert!(tracker.is_dirty(4));
        assert_eq!(tracker.dirty_count(), 3);

        // Shrink
        tracker.resize(2);
        assert_eq!(tracker.dirty_count(), 1);
        assert!(!tracker.is_dirty(2)); // out of bounds => false
    }

    // -- inline decoration placement tests ----------------------------------

    #[test]
    fn resolve_decoration_columns_plain() {
        let dec = LineDecoration::new(2, 5, DecorationKind::Error);
        let resolved = resolve_decoration_columns("hello world", &dec, 4);
        assert_eq!(resolved.display_start, 2);
        assert_eq!(resolved.display_end, 5);
        assert_eq!(resolved.kind, DecorationKind::Error);
    }

    #[test]
    fn resolve_decoration_columns_with_tab() {
        // Line: \thello   (tab_width=4 => \t occupies cols 0-3, 'h' at col 4)
        // Decoration on char indices 1..4 => "hel" => display cols 4..7
        let dec = LineDecoration::new(1, 4, DecorationKind::SearchMatch);
        let resolved = resolve_decoration_columns("\thello", &dec, 4);
        assert_eq!(resolved.display_start, 4);
        assert_eq!(resolved.display_end, 7);
    }

    // -- cursor blink state machine tests -----------------------------------

    #[test]
    fn cursor_blink_initial_state() {
        let blink = CursorBlinkState::new();
        assert_eq!(blink.phase, BlinkPhase::Visible);
        assert!(blink.should_draw());
        assert_eq!(blink.elapsed_ms, 0);
    }

    #[test]
    fn cursor_blink_tick_transitions() {
        let mut blink = CursorBlinkState::new();
        // Tick less than threshold: no change
        assert!(!blink.tick(100));
        assert_eq!(blink.phase, BlinkPhase::Visible);

        // Tick past visible threshold
        assert!(blink.tick(500)); // total 600 >= 530
        assert_eq!(blink.phase, BlinkPhase::Hidden);
        assert!(!blink.should_draw());

        // Tick past hidden threshold
        assert!(blink.tick(530));
        assert_eq!(blink.phase, BlinkPhase::Visible);
        assert!(blink.should_draw());
    }

    #[test]
    fn cursor_blink_reset_on_input() {
        let mut blink = CursorBlinkState::new();
        blink.tick(600); // => Hidden
        assert_eq!(blink.phase, BlinkPhase::Hidden);

        blink.reset_on_input();
        assert_eq!(blink.phase, BlinkPhase::Paused);
        assert!(blink.should_draw()); // Paused is still drawable

        // After pause_ms, transitions to Visible
        assert!(blink.tick(800));
        assert_eq!(blink.phase, BlinkPhase::Visible);
    }

    #[test]
    fn cursor_blink_disabled() {
        let mut blink = CursorBlinkState::new();
        blink.enabled = false;
        assert!(blink.should_draw()); // always drawn when disabled
        assert!(!blink.tick(10000)); // tick never changes phase
        assert_eq!(blink.phase, BlinkPhase::Visible);
    }
}
