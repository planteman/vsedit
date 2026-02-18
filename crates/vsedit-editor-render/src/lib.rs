//! Terminal editor line rendering.
//!
//! Provides viewport-aware rendering of editor lines with decoration merging
//! for selections, search highlights, diagnostics, and other visual markers.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// RenderLineDecoration – overlay decoration collector
// ---------------------------------------------------------------------------

/// Kind of overlay decoration applied to a rendered line region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayDecorationKind {
    Underline,
    Highlight,
    InlineText,
    Gutter,
}

impl fmt::Display for OverlayDecorationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Underline => write!(f, "Underline"),
            Self::Highlight => write!(f, "Highlight"),
            Self::InlineText => write!(f, "InlineText"),
            Self::Gutter => write!(f, "Gutter"),
        }
    }
}

/// A single overlay decoration entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayDecoration {
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub kind: OverlayDecorationKind,
    pub text: Option<String>,
}

/// Collects and queries overlay decorations across lines.
#[derive(Debug, Clone, Default)]
pub struct RenderLineDecoration {
    pub decorations: Vec<OverlayDecoration>,
}

impl RenderLineDecoration {
    pub fn new() -> Self {
        Self {
            decorations: Vec::new(),
        }
    }

    pub fn add(&mut self, dec: OverlayDecoration) {
        self.decorations.push(dec);
    }

    pub fn decorations_for_line(&self, line: u32) -> Vec<&OverlayDecoration> {
        self.decorations.iter().filter(|d| d.line == line).collect()
    }

    pub fn has_decorations(&self, line: u32) -> bool {
        self.decorations.iter().any(|d| d.line == line)
    }

    pub fn remove_for_line(&mut self, line: u32) {
        self.decorations.retain(|d| d.line != line);
    }

    pub fn total_count(&self) -> usize {
        self.decorations.len()
    }

    pub fn clear(&mut self) {
        self.decorations.clear();
    }
}

// ---------------------------------------------------------------------------
// RenderWhitespaceTokenizer – visible whitespace rendering
// ---------------------------------------------------------------------------

/// Classification of a whitespace region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    Space,
    Tab,
    TrailingSpace,
}

impl fmt::Display for WhitespaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Space => write!(f, "Space"),
            Self::Tab => write!(f, "Tab"),
            Self::TrailingSpace => write!(f, "TrailingSpace"),
        }
    }
}

/// A contiguous whitespace region inside a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceToken {
    pub kind: WhitespaceKind,
    pub start: usize,
    pub len: usize,
}

/// Tokenizes and renders visible whitespace indicators.
pub struct RenderWhitespaceTokenizer;

impl RenderWhitespaceTokenizer {
    /// Splits `line` into whitespace tokens preserving their byte offsets.
    pub fn tokenize(line: &str) -> Vec<WhitespaceToken> {
        let trimmed = line.trim_end_matches(|c: char| c == ' ' || c == '\t');
        let trailing_start = trimmed.len();
        let mut tokens = Vec::new();
        let mut idx = 0;
        for ch in line.chars() {
            let clen = ch.len_utf8();
            match ch {
                '\t' => {
                    let kind = if idx >= trailing_start {
                        WhitespaceKind::TrailingSpace
                    } else {
                        WhitespaceKind::Tab
                    };
                    tokens.push(WhitespaceToken { kind, start: idx, len: clen });
                }
                ' ' => {
                    let kind = if idx >= trailing_start {
                        WhitespaceKind::TrailingSpace
                    } else {
                        WhitespaceKind::Space
                    };
                    tokens.push(WhitespaceToken { kind, start: idx, len: clen });
                }
                _ => {}
            }
            idx += clen;
        }
        tokens
    }

    /// Returns a copy of `line` with whitespace replaced by visible indicators.
    /// Spaces → `·`, tabs → `→`, trailing spaces → `•`.
    pub fn render_visible(line: &str) -> String {
        let trimmed_len = line.trim_end_matches(|c: char| c == ' ' || c == '\t').len();
        let mut out = String::with_capacity(line.len() * 2);
        for (i, ch) in line.char_indices() {
            match ch {
                ' ' if i >= trimmed_len => out.push('•'),
                ' ' => out.push('·'),
                '\t' if i >= trimmed_len => out.push('•'),
                '\t' => out.push('→'),
                other => out.push(other),
            }
        }
        out
    }

    /// Returns `true` when `line` ends with whitespace.
    pub fn has_trailing_whitespace(line: &str) -> bool {
        line.ends_with(' ') || line.ends_with('\t')
    }

    /// Number of leading whitespace characters.
    pub fn leading_whitespace_len(line: &str) -> usize {
        line.chars().take_while(|c| c.is_whitespace()).count()
    }
}

// ---------------------------------------------------------------------------
// RenderMinimapLine – braille-based minimap rendering
// ---------------------------------------------------------------------------

/// Renders simplified braille-character minimap lines.
pub struct RenderMinimapLine;

impl RenderMinimapLine {
    pub fn new() -> Self {
        Self
    }

    /// Renders `line` as braille characters of the given `width`.
    /// Non-space characters map to `⣿`, spaces to `⠀` (braille blank).
    pub fn render_braille(line: &str, width: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let chars: Vec<char> = line.chars().collect();
        let mut out = String::with_capacity(width * 3);
        for i in 0..width {
            let ch = chars.get(i).copied().unwrap_or(' ');
            if ch != ' ' && ch != '\t' {
                out.push('⣿');
            } else {
                out.push('⠀');
            }
        }
        out
    }

    /// Ratio of non-whitespace characters to total length (`0.0`–`1.0`).
    pub fn line_density(line: &str) -> f32 {
        if line.is_empty() {
            return 0.0;
        }
        let total = line.len() as f32;
        let non_ws = line.chars().filter(|c| !c.is_whitespace()).count() as f32;
        non_ws / total
    }

    /// Renders a block of source lines into minimap braille strings.
    pub fn render_minimap_block(lines: &[&str], width: usize) -> Vec<String> {
        lines.iter().map(|l| Self::render_braille(l, width)).collect()
    }
}

impl Default for RenderMinimapLine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LineDirtyTracker – per-line dirty state with generation counter
// ---------------------------------------------------------------------------

/// Tracks which lines need re-rendering and exposes a generation counter
/// that increments on bulk operations.
#[derive(Debug, Clone)]
pub struct LineDirtyTracker {
    dirty_lines: HashMap<u32, bool>,
    generation: u64,
}

impl LineDirtyTracker {
    pub fn new() -> Self {
        Self {
            dirty_lines: HashMap::new(),
            generation: 0,
        }
    }

    pub fn mark_dirty(&mut self, line: u32) {
        self.dirty_lines.insert(line, true);
    }

    pub fn mark_clean(&mut self, line: u32) {
        self.dirty_lines.remove(&line);
    }

    pub fn is_dirty(&self, line: u32) -> bool {
        self.dirty_lines.get(&line).copied().unwrap_or(false)
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_lines.len()
    }

    pub fn all_dirty_lines(&self) -> Vec<u32> {
        let mut lines: Vec<u32> = self.dirty_lines.keys().copied().collect();
        lines.sort_unstable();
        lines
    }

    pub fn mark_all_clean(&mut self) {
        self.dirty_lines.clear();
    }

    /// Increments and returns the new generation value.
    pub fn bump_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl Default for LineDirtyTracker {
    fn default() -> Self {
        Self::new()
    }
}


// ─── Render Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for rendered frames.
#[derive(Debug, Clone)]
pub struct RenderRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> RenderRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for RenderRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RenderRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Render LRU Cache ───────────────────────────────────────

/// A simple LRU cache for render cache.
#[derive(Debug)]
pub struct RenderLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> RenderLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for RenderLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RenderLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


// ---------------------------------------------------------------------------
// editor_render – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XEditorRenderTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XEditorRenderTextSpan {
    pub fn new(start: usize, end: usize) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self { start: s, end: e }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extract the spanned slice from `text`.
    pub fn extract<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }

    /// Returns true if `pos` is contained within this span.
    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns the overlap with `other`, if any.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let s = self.start.max(other.start);
        let e = self.end.min(other.end);
        if s < e { Some(Self { start: s, end: e }) } else { None }
    }

    /// Merge two spans into the smallest enclosing span.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Shift the span by `delta` positions to the right.
    pub fn shift(&self, delta: usize) -> Self {
        Self { start: self.start + delta, end: self.end + delta }
    }
}

/// Count the number of lines in `text`.
pub fn x_editor_render_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_editor_render_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_editor_render_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_editor_render_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_editor_render_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_editor_render_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}


/// Configuration manager for editor_render functionality.
pub struct EditorRenderConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl EditorRenderConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &EditorRenderConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for editor_render operations.
pub struct EditorRenderRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl EditorRenderRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for editor_render.
pub struct EditorRenderValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl EditorRenderValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &EditorRenderValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 13
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer13 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer13 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_13(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_13<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_13<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_13(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_13(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 36
// ---------------------------------------------------------------------------

/// Generic object pool `Xc36Pool<T>`.
pub struct Xc36Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc36Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc36PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc36Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc36PoolStats {
        Xc36PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc36Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc36Scheduler`.
pub struct Xc36Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc36Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc36Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_36 hash for the given byte slice.
pub fn xc_36_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_36 convention.
pub fn xc_36_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe25 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe25Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe25PipelineError {
    pub stage: Xe25Stage,
    pub message: String,
}

impl std::fmt::Display for Xe25PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe25Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe25Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError>>>,
    stage_names: Vec<Xe25Stage>,
}

impl Xe25Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe25Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe25Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe25Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe25Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe25Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe25CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe25CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe25Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe25CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe25CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe25Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe25CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_25_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe25CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_25_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe25CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_25_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
    Ok(data)
}

pub fn xe_25_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_25_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_25_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_25_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe25PipelineError> {
    Err(Xe25PipelineError {
        stage: Xe25Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #110
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf110Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf110TrieNode {
    children: std::collections::HashMap<char, Xf110TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf110Trie {
    root: Xf110TrieNode,
    count: usize,
}

impl Xf110Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf110TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf110TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf110TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf110BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf110BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 35).
pub struct Xh35SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh35SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 77 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 35).
pub struct Xh35BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh35BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 35).
pub struct Xi35Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi35Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi35Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi35Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 35).
pub struct Xi35IntervalTree {
    xi_intervals: Vec<Xi35Interval>,
}

impl Xi35IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi35Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi35Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi35Interval) -> Vec<&Xi35Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi35Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi35Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi35Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi35Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi35Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi35Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 35) ---

/// Disjoint set / union-find for crate 35.
pub struct Xj35UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj35UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ35_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 35.
pub struct Xj35BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj35BTreeNode<K, V>>>,
    len: usize,
}

struct Xj35BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj35BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj35BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ35_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ35_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj35BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj35BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj35BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj35BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_35 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk35SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk35SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk35DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk35DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_35).
#[derive(Debug, Clone)]
pub struct Xl35Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl35Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_35).
#[derive(Debug, Clone)]
pub struct Xl35SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl35SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm35MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm35MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm35Tokenizer {
    text: String,
}

impl Xm35Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 35.
pub struct Xn35Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn35Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 35 -----

#[derive(Debug, Clone)]
struct Xn35AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn35AvlNode<K, V>>>,
    right: Option<Box<Xn35AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 35.
#[derive(Debug, Clone)]
pub struct Xn35AVL<K, V> {
    root: Option<Box<Xn35AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn35AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn35AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn35AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn35AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn35AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn35AvlNode<K, V>>) -> Box<Xn35AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn35AvlNode<K, V>>) -> Box<Xn35AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn35AvlNode<K, V>>) -> Box<Xn35AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn35AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn35AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn35AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn35AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn35AvlNode<K, V>>) -> &Xn35AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn35AvlNode<K, V>>) -> (Box<Xn35AvlNode<K, V>>, Option<Box<Xn35AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn35AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn35AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn35AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn35AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn35AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn35AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn35AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo35RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo35Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo35RBNode<K, V> {
    key: K,
    value: V,
    color: Xo35Color,
    left: Option<Box<Xo35RBNode<K, V>>>,
    right: Option<Box<Xo35RBNode<K, V>>>,
}

/// A red-black tree map for crate 35.
#[derive(Debug, Clone)]
pub struct Xo35RedBlack<K, V> {
    root: Option<Box<Xo35RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo35RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo35Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo35RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo35RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo35RBNode {
                    key, value, color: Xo35Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo35RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo35Color::Red)
    }

    fn xo_balance(mut h: Box<Xo35RBNode<K, V>>) -> Box<Xo35RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo35Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo35RBNode<K, V>>) -> Box<Xo35RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo35Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo35RBNode<K, V>>) -> Box<Xo35RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo35Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo35RBNode<K, V>>) {
        h.color = Xo35Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo35Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo35Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo35Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo35RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo35RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo35RBNode<K, V>) -> (K, V, Option<Box<Xo35RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo35RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo35Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo35RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo35ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 35.
#[derive(Debug, Clone)]
pub struct Xo35ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo35ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo35#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo35#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
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

    // -- RenderLineDecoration tests -----------------------------------------

    #[test]
    fn render_line_decoration_add_and_query() {
        let mut rld = RenderLineDecoration::new();
        assert_eq!(rld.total_count(), 0);
        assert!(!rld.has_decorations(1));

        rld.add(OverlayDecoration {
            line: 1,
            start_col: 0,
            end_col: 5,
            kind: OverlayDecorationKind::Underline,
            text: None,
        });
        rld.add(OverlayDecoration {
            line: 1,
            start_col: 6,
            end_col: 10,
            kind: OverlayDecorationKind::Highlight,
            text: None,
        });
        rld.add(OverlayDecoration {
            line: 2,
            start_col: 0,
            end_col: 3,
            kind: OverlayDecorationKind::Gutter,
            text: Some("!".to_string()),
        });

        assert_eq!(rld.total_count(), 3);
        assert!(rld.has_decorations(1));
        assert!(rld.has_decorations(2));
        assert!(!rld.has_decorations(3));

        let line1 = rld.decorations_for_line(1);
        assert_eq!(line1.len(), 2);
        assert_eq!(line1[0].kind, OverlayDecorationKind::Underline);
    }

    #[test]
    fn render_line_decoration_remove_and_clear() {
        let mut rld = RenderLineDecoration::new();
        rld.add(OverlayDecoration {
            line: 1, start_col: 0, end_col: 5,
            kind: OverlayDecorationKind::InlineText,
            text: Some("hint".to_string()),
        });
        rld.add(OverlayDecoration {
            line: 2, start_col: 0, end_col: 3,
            kind: OverlayDecorationKind::Highlight,
            text: None,
        });
        rld.remove_for_line(1);
        assert!(!rld.has_decorations(1));
        assert_eq!(rld.total_count(), 1);

        rld.clear();
        assert_eq!(rld.total_count(), 0);
    }

    #[test]
    fn overlay_decoration_kind_display() {
        assert_eq!(OverlayDecorationKind::Underline.to_string(), "Underline");
        assert_eq!(OverlayDecorationKind::Highlight.to_string(), "Highlight");
        assert_eq!(OverlayDecorationKind::InlineText.to_string(), "InlineText");
        assert_eq!(OverlayDecorationKind::Gutter.to_string(), "Gutter");
    }

    // -- RenderWhitespaceTokenizer tests ------------------------------------

    #[test]
    fn whitespace_tokenize_basic() {
        let tokens = RenderWhitespaceTokenizer::tokenize("  hello  ");
        assert!(tokens.len() >= 4);
        assert_eq!(tokens[0].kind, WhitespaceKind::Space);
        assert_eq!(tokens[1].kind, WhitespaceKind::Space);
        // last two spaces are trailing
        let trailing: Vec<_> = tokens.iter().filter(|t| t.kind == WhitespaceKind::TrailingSpace).collect();
        assert_eq!(trailing.len(), 2);
    }

    #[test]
    fn whitespace_tokenize_tabs() {
        let tokens = RenderWhitespaceTokenizer::tokenize("\thello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, WhitespaceKind::Tab);
        assert_eq!(tokens[0].start, 0);
    }

    #[test]
    fn whitespace_render_visible() {
        let vis = RenderWhitespaceTokenizer::render_visible("  hi  ");
        assert_eq!(vis, "··hi••");
    }

    #[test]
    fn whitespace_render_visible_tabs() {
        let vis = RenderWhitespaceTokenizer::render_visible("\thi");
        assert_eq!(vis, "→hi");
    }

    #[test]
    fn whitespace_has_trailing() {
        assert!(RenderWhitespaceTokenizer::has_trailing_whitespace("hello "));
        assert!(RenderWhitespaceTokenizer::has_trailing_whitespace("hello\t"));
        assert!(!RenderWhitespaceTokenizer::has_trailing_whitespace("hello"));
        assert!(!RenderWhitespaceTokenizer::has_trailing_whitespace(""));
    }

    #[test]
    fn whitespace_leading_len() {
        assert_eq!(RenderWhitespaceTokenizer::leading_whitespace_len("   abc"), 3);
        assert_eq!(RenderWhitespaceTokenizer::leading_whitespace_len("\tabc"), 1);
        assert_eq!(RenderWhitespaceTokenizer::leading_whitespace_len("abc"), 0);
        assert_eq!(RenderWhitespaceTokenizer::leading_whitespace_len(""), 0);
    }

    #[test]
    fn whitespace_kind_display() {
        assert_eq!(WhitespaceKind::Space.to_string(), "Space");
        assert_eq!(WhitespaceKind::Tab.to_string(), "Tab");
        assert_eq!(WhitespaceKind::TrailingSpace.to_string(), "TrailingSpace");
    }

    // -- RenderMinimapLine tests --------------------------------------------

    #[test]
    fn minimap_render_braille_basic() {
        let result = RenderMinimapLine::render_braille("hello", 5);
        assert_eq!(result, "⣿⣿⣿⣿⣿");
    }

    #[test]
    fn minimap_render_braille_with_spaces() {
        let result = RenderMinimapLine::render_braille("a b", 4);
        assert_eq!(result, "⣿⠀⣿⠀"); // 'a' ' ' 'b' padding-space
    }

    #[test]
    fn minimap_render_braille_zero_width() {
        assert_eq!(RenderMinimapLine::render_braille("hello", 0), "");
    }

    #[test]
    fn minimap_line_density() {
        let d = RenderMinimapLine::line_density("hello");
        assert!((d - 1.0).abs() < f32::EPSILON);

        let d2 = RenderMinimapLine::line_density("  ");
        assert!((d2 - 0.0).abs() < f32::EPSILON);

        assert!((RenderMinimapLine::line_density("") - 0.0).abs() < f32::EPSILON);

        let d3 = RenderMinimapLine::line_density("ab cd");
        // 4 non-ws out of 5 chars
        assert!((d3 - 0.8).abs() < 0.01);
    }

    #[test]
    fn minimap_render_block() {
        let lines = vec!["abc", "   ", "x"];
        let block = RenderMinimapLine::render_minimap_block(&lines, 3);
        assert_eq!(block.len(), 3);
        assert_eq!(block[0], "⣿⣿⣿");
        assert_eq!(block[1], "⠀⠀⠀");
        assert_eq!(block[2], "⣿⠀⠀");
    }

    // -- LineDirtyTracker tests ---------------------------------------------

    #[test]
    fn line_dirty_tracker_basic() {
        let mut tracker = LineDirtyTracker::new();
        assert_eq!(tracker.dirty_count(), 0);
        assert_eq!(tracker.generation(), 0);

        tracker.mark_dirty(5);
        tracker.mark_dirty(10);
        assert!(tracker.is_dirty(5));
        assert!(tracker.is_dirty(10));
        assert!(!tracker.is_dirty(7));
        assert_eq!(tracker.dirty_count(), 2);
        assert_eq!(tracker.all_dirty_lines(), vec![5, 10]);
    }

    #[test]
    fn line_dirty_tracker_clean_and_generation() {
        let mut tracker = LineDirtyTracker::new();
        tracker.mark_dirty(1);
        tracker.mark_dirty(2);
        tracker.mark_dirty(3);
        tracker.mark_clean(2);
        assert!(!tracker.is_dirty(2));
        assert_eq!(tracker.dirty_count(), 2);

        tracker.mark_all_clean();
        assert_eq!(tracker.dirty_count(), 0);

        let g = tracker.bump_generation();
        assert_eq!(g, 1);
        assert_eq!(tracker.generation(), 1);
        let g2 = tracker.bump_generation();
        assert_eq!(g2, 2);
    }

    #[test]
    fn render_ringbuf_push_get() {
        let mut rb = RenderRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn render_ringbuf_overflow() {
        let mut rb = RenderRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn render_ringbuf_clear() {
        let mut rb = RenderRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn render_ringbuf_newest_oldest() {
        let mut rb = RenderRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn render_ringbuf_to_vec() {
        let mut rb = RenderRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn render_ringbuf_is_full() {
        let mut rb = RenderRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn render_lru_insert_get() {
        let mut c = RenderLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn render_lru_eviction() {
        let mut c = RenderLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn render_lru_hit_ratio() {
        let mut c = RenderLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn render_lru_clear() {
        let mut c = RenderLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn render_lru_remove() {
        let mut c = RenderLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn render_lru_peek() {
        let mut c = RenderLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    // -- editor_render additional tests -------------------------------------------

    #[test]
    fn x_editor_render_text_span_new_ordered() {
        let s = XEditorRenderTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_editor_render_text_span_new_reversed() {
        let s = XEditorRenderTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_editor_render_text_span_len() {
        assert_eq!(XEditorRenderTextSpan::new(3, 7).len(), 4);
        assert_eq!(XEditorRenderTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_editor_render_text_span_extract() {
        let s = XEditorRenderTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_editor_render_text_span_contains() {
        let s = XEditorRenderTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_editor_render_text_span_intersect() {
        let a = XEditorRenderTextSpan::new(0, 10);
        let b = XEditorRenderTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_editor_render_text_span_intersect_none() {
        let a = XEditorRenderTextSpan::new(0, 5);
        let b = XEditorRenderTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_editor_render_text_span_union() {
        let a = XEditorRenderTextSpan::new(3, 7);
        let b = XEditorRenderTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_editor_render_count_lines_basic() {
        assert_eq!(x_editor_render_count_lines("a\nb\nc"), 3);
        assert_eq!(x_editor_render_count_lines(""), 0);
        assert_eq!(x_editor_render_count_lines("single"), 1);
    }

    #[test]
    fn x_editor_render_line_start_offset_basic() {
        assert_eq!(x_editor_render_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_editor_render_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_editor_render_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_editor_render_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_editor_render_indent_level_basic() {
        assert_eq!(x_editor_render_indent_level("    hello"), 4);
        assert_eq!(x_editor_render_indent_level("hello"), 0);
        assert_eq!(x_editor_render_indent_level("  "), 2);
    }

    #[test]
    fn x_editor_render_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_editor_render_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_editor_render_detect_eol_lf() {
        assert_eq!(x_editor_render_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_editor_render_detect_eol_crlf() {
        assert_eq!(x_editor_render_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_editor_render_tokenize_basic() {
        let tokens = x_editor_render_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_editor_render_text_span_shift() {
        let s = XEditorRenderTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    #[test]
    fn editor_render_config_new() {
        let cfg = EditorRenderConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn editor_render_config_set_get() {
        let mut cfg = EditorRenderConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn editor_render_config_remove() {
        let mut cfg = EditorRenderConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn editor_render_config_keys_sorted() {
        let mut cfg = EditorRenderConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn editor_render_config_bump_version() {
        let mut cfg = EditorRenderConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn editor_render_config_clear() {
        let mut cfg = EditorRenderConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn editor_render_config_merge() {
        let mut cfg1 = EditorRenderConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = EditorRenderConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn editor_render_config_disable() {
        let mut cfg = EditorRenderConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn editor_render_rate_tracker_empty() {
        let rt = EditorRenderRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn editor_render_rate_tracker_record() {
        let mut rt = EditorRenderRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn editor_render_rate_tracker_prune() {
        let mut rt = EditorRenderRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn editor_render_validator_valid() {
        let v = EditorRenderValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn editor_render_validator_errors() {
        let mut v = EditorRenderValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn editor_render_validator_clear() {
        let mut v = EditorRenderValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn editor_render_validator_merge() {
        let mut v1 = EditorRenderValidator::new();
        v1.add_error("e1");
        let mut v2 = EditorRenderValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn editor_render_rate_tracker_clear() {
        let mut rt = EditorRenderRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_13_push_and_len() {
        let mut rb = super::XbRingBuffer13::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_13_overwrite() {
        let mut rb = super::XbRingBuffer13::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_13_get_out_of_bounds() {
        let rb = super::XbRingBuffer13::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_13_drain_all() {
        let mut rb = super::XbRingBuffer13::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_13_peek_front_back() {
        let mut rb = super::XbRingBuffer13::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_13_clear() {
        let mut rb = super::XbRingBuffer13::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_13_capacity() {
        let rb = super::XbRingBuffer13::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_13_basic() {
        let h = super::xb_fnv1a_13(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_13(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_13_different_inputs() {
        let h1 = super::xb_fnv1a_13(b"abc");
        let h2 = super::xb_fnv1a_13(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_13_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_13(&data);
        let dec = super::xb_rle_decode_13(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_13_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_13(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_13(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_13_values() {
        assert!((super::xb_clamp_13(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_13(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_13(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_13_values() {
        assert!((super::xb_lerp_13(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_13(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_13(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_13_wrap_around_twice() {
        let mut rb = super::XbRingBuffer13::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 36 ----

    #[test]
    fn xc_36_pool_new_empty() {
        let pool: super::Xc36Pool<i32> = super::Xc36Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_36_pool_release_acquire() {
        let mut pool = super::Xc36Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_36_pool_acquire_empty() {
        let mut pool: super::Xc36Pool<i32> = super::Xc36Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_36_pool_full() {
        let mut pool = super::Xc36Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_36_pool_drain() {
        let mut pool = super::Xc36Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_36_pool_stats() {
        let mut pool = super::Xc36Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_36_pool_clear() {
        let mut pool = super::Xc36Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_36_pool_shrink() {
        let mut pool = super::Xc36Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_36_pool_default() {
        let pool: super::Xc36Pool<String> = super::Xc36Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_36_pool_extend() {
        let mut pool = super::Xc36Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_36_pool_retain() {
        let mut pool = super::Xc36Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_36_scheduler_round_robin() {
        let mut sched = super::Xc36Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_36_scheduler_empty() {
        let mut sched = super::Xc36Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_36_scheduler_reset() {
        let mut sched = super::Xc36Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_36_scheduler_add_remove() {
        let mut sched = super::Xc36Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_36_scheduler_targets() {
        let sched = super::Xc36Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_36_hash_empty() {
        assert_eq!(super::xc_36_hash(b""), 5381);
    }

    #[test]
    fn xc_36_hash_data() {
        let h = super::xc_36_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_36_hash(b"hello"), h);
    }

    #[test]
    fn xc_36_reverse_str() {
        assert_eq!(super::xc_36_reverse("abc"), "cba");
        assert_eq!(super::xc_36_reverse(""), "");
    }


    #[test]
    fn xe_25_pipeline_empty() {
        let p = super::Xe25Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_25_pipeline_parse_stage() {
        let p = super::Xe25Pipeline::new()
            .add_parse(super::xe_25_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_25_pipeline_transform_double() {
        let p = super::Xe25Pipeline::new()
            .add_transform(super::xe_25_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_25_pipeline_validate_reverse() {
        let p = super::Xe25Pipeline::new()
            .add_validate(super::xe_25_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_25_pipeline_emit_filter() {
        let p = super::Xe25Pipeline::new()
            .add_emit(super::xe_25_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_25_pipeline_multi_stage() {
        let p = super::Xe25Pipeline::new()
            .add_parse(super::xe_25_pipeline_identity)
            .add_transform(super::xe_25_pipeline_double)
            .add_validate(super::xe_25_pipeline_reverse)
            .add_emit(super::xe_25_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_25_pipeline_error_propagation() {
        let p = super::Xe25Pipeline::new()
            .add_parse(super::xe_25_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe25Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_25_pipeline_compose() {
        let p1 = super::Xe25Pipeline::new()
            .add_parse(super::xe_25_pipeline_identity);
        let p2 = super::Xe25Pipeline::new()
            .add_transform(super::xe_25_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_25_pipeline_error_display() {
        let e = super::Xe25PipelineError {
            stage: super::Xe25Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_25_cache_put_get() {
        let mut c = super::Xe25Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_25_cache_miss() {
        let mut c: super::Xe25Cache<&str, i32> = super::Xe25Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_25_cache_ttl_expiry() {
        let mut c = super::Xe25Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_25_cache_evict() {
        let mut c = super::Xe25Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_25_cache_capacity() {
        let mut c = super::Xe25Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_25_cache_stats() {
        let mut c = super::Xe25Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_25_cache_clear() {
        let mut c = super::Xe25Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #110 --

    #[test]
    fn xf110_trie_insert_search() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf110_trie_starts_with() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf110_trie_remove() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf110_trie_word_count() {
        let mut t = Xf110Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf110_trie_longest_prefix() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf110_trie_all_words() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf110_trie_autocomplete() {
        let mut t = Xf110Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf110_trie_empty_search() {
        let t = Xf110Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf110_bloom_add_contains() {
        let mut bf = Xf110BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf110_bloom_probably_absent() {
        let bf = Xf110BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf110_bloom_false_positive_rate() {
        let mut bf = Xf110BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf110_bloom_clear() {
        let mut bf = Xf110BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf110_bloom_union() {
        let mut a = Xf110BloomFilter::xf_new(512, 2);
        let mut b = Xf110BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf110_bloom_intersection_estimate() {
        let mut a = Xf110BloomFilter::xf_new(512, 2);
        let mut b = Xf110BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf110_bloom_union_size_mismatch() {
        let a = Xf110BloomFilter::xf_new(256, 2);
        let b = Xf110BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh35_skip_insert_contains() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh35_skip_remove() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh35_skip_len() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh35_skip_range_query() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh35_skip_floor_ceiling() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh35_skip_rank() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh35_skip_empty() {
        let sl = super::Xh35SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh35_skip_duplicates() {
        let mut sl = super::Xh35SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh35_bitset_set_test() {
        let mut bs = super::Xh35BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh35_bitset_clear_count() {
        let mut bs = super::Xh35BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh35_bitset_and_or_xor() {
        let mut a = super::Xh35BitSet::xh_new(128);
        let mut b = super::Xh35BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh35_bitset_iter_ones() {
        let mut bs = super::Xh35BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh35_bitset_first_last() {
        let mut bs = super::Xh35BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh35_bitset_empty() {
        let bs = super::Xh35BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi35_deque_push_pop_back() {
        let mut dq = super::Xi35Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi35_deque_push_pop_front() {
        let mut dq = super::Xi35Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi35_deque_mixed_ops() {
        let mut dq = super::Xi35Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi35_deque_get_and_split() {
        let mut dq = super::Xi35Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi35_deque_rotate_left() {
        let mut dq = super::Xi35Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi35_deque_rotate_right() {
        let mut dq = super::Xi35Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi35_deque_grow() {
        let mut dq = super::Xi35Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi35_deque_empty() {
        let dq = super::Xi35Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi35_interval_tree_insert_query() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi35Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi35Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi35_interval_tree_overlap() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi35Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi35Interval::xi_new(12, 20));
        let q = super::Xi35Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi35_interval_tree_remove() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi35Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi35_interval_tree_gaps() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi35Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi35Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi35Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi35Interval::xi_new(8, 10));
    }

    #[test]
    fn xi35_interval_tree_merge() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi35Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi35Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi35Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi35Interval::xi_new(10, 15));
    }

    #[test]
    fn xi35_interval_tree_all() {
        let mut tree = super::Xi35IntervalTree::xi_new();
        tree.xi_insert(super::Xi35Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi35Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi35_interval_tree_empty() {
        let tree = super::Xi35IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi35_interval_tree_contains_point() {
        let iv = super::Xi35Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 35) ---

    #[test]
    fn xj_35_uf_make_and_find() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_35_uf_union_connected() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_35_uf_component_count() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_35_uf_component_size() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_35_uf_largest_component() {
        let mut uf = super::Xj35UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_35_uf_many_elements() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_35_uf_separate_components() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_35_uf_path_compression() {
        let mut uf = super::Xj35UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_35_bt_insert_get() {
        let mut bt = super::Xj35BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_35_bt_contains_len() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_35_bt_replace() {
        let mut bt = super::Xj35BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_35_bt_remove() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_35_bt_keys_values() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_35_bt_range() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_35_bt_min_max() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_35_bt_many_inserts() {
        let mut bt = super::Xj35BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_35 segment tree tests ---

    #[test]
    fn xk_35_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_35_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk35SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_35_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_35_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_35_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_35_st_single_element() {
        let data = vec![42];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_35_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk35SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_35_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk35SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_35 disjoint intervals tests ---

    #[test]
    fn xk_35_di_add_and_count() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_35_di_merge_overlap() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_35_di_contains() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_35_di_remove() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_35_di_covered_length() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_35_di_gaps() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_35_di_merge_adjacent() {
        let mut di = super::Xk35DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_35_di_empty() {
        let di = super::Xk35DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_35_rope_new_empty() {
        let rope = super::Xl35Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_35_rope_from_str() {
        let rope = super::Xl35Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_35_rope_insert_at() {
        let mut rope = super::Xl35Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_35_rope_delete_range() {
        let mut rope = super::Xl35Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_35_rope_char_at() {
        let rope = super::Xl35Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_35_rope_split_concat() {
        let rope = super::Xl35Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_35_rope_line_count() {
        let rope = super::Xl35Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_35_rope_line_at() {
        let rope = super::Xl35Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_35_sa_build_and_search() {
        let sa = super::Xl35SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_35_sa_count() {
        let sa = super::Xl35SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_35_sa_longest_repeated() {
        let sa = super::Xl35SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_35_sa_all_positions() {
        let sa = super::Xl35SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_35_sa_len() {
        let sa = super::Xl35SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_35_sa_empty() {
        let sa = super::Xl35SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_35_rope_slice() {
        let rope = super::Xl35Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_35_sa_search_start() {
        let sa = super::Xl35SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_35_sparse_set_get() {
        let mut m = super::Xm35MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_35_sparse_row_col() {
        let mut m = super::Xm35MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_35_sparse_transpose() {
        let mut m = super::Xm35MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_35_sparse_multiply_vec() {
        let mut m = super::Xm35MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_35_sparse_nnz_density() {
        let mut m = super::Xm35MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_35_sparse_clear() {
        let mut m = super::Xm35MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_35_sparse_overwrite_zero() {
        let mut m = super::Xm35MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_35_tokenizer_basic() {
        let t = super::Xm35Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_35_tokenizer_count() {
        let t = super::Xm35Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_35_tokenizer_unique() {
        let t = super::Xm35Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_35_tokenizer_frequency() {
        let t = super::Xm35Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_35_tokenizer_delimiter() {
        let t = super::Xm35Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_35_tokenizer_whitespace() {
        let t = super::Xm35Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_35_tokenizer_empty() {
        let t = super::Xm35Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 35 ----

    #[test]
    fn xn_35_fenwick_prefix_sum() {
        let mut ft = super::Xn35Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_35_fenwick_range_sum() {
        let mut ft = super::Xn35Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_35_fenwick_point_query() {
        let mut ft = super::Xn35Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_35_fenwick_len() {
        let ft = super::Xn35Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_35_fenwick_multiple_updates() {
        let mut ft = super::Xn35Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_35_fenwick_single_element() {
        let mut ft = super::Xn35Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_35_fenwick_find_kth() {
        let mut ft = super::Xn35Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_35_fenwick_negative_delta() {
        let mut ft = super::Xn35Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 35 ----

    #[test]
    fn xn_35_avl_insert_get() {
        let mut m = super::Xn35AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_35_avl_remove() {
        let mut m = super::Xn35AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_35_avl_in_order() {
        let mut m = super::Xn35AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_35_avl_min_max() {
        let mut m = super::Xn35AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_35_avl_floor_ceiling() {
        let mut m = super::Xn35AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_35_avl_height_balanced() {
        let mut m = super::Xn35AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_35_avl_overwrite() {
        let mut m = super::Xn35AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_35_avl_empty() {
        let m: super::Xn35AVL<i32, i32> = super::Xn35AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo35RedBlack tests ---

    #[test]
    fn xo_35_rb_insert_and_get() {
        let mut tree = super::Xo35RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_35_rb_len_and_empty() {
        let mut tree = super::Xo35RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_35_rb_min_max() {
        let mut tree = super::Xo35RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_35_rb_contains() {
        let mut tree = super::Xo35RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_35_rb_remove() {
        let mut tree = super::Xo35RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_35_rb_in_order() {
        let mut tree = super::Xo35RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_35_rb_black_height() {
        let mut tree = super::Xo35RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_35_rb_overwrite() {
        let mut tree = super::Xo35RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo35ConsistentHash tests ---

    #[test]
    fn xo_35_ch_add_and_count() {
        let mut ring = super::Xo35ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_35_ch_remove_node() {
        let mut ring = super::Xo35ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_35_ch_get_node() {
        let mut ring = super::Xo35ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_35_ch_empty_ring() {
        let ring = super::Xo35ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_35_ch_distribution() {
        let mut ring = super::Xo35ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_35_ch_rebalance() {
        let mut ring = super::Xo35ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_35_ch_virtual_nodes() {
        let mut ring = super::Xo35ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_35_ch_consistent_lookup() {
        let mut ring = super::Xo35ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}