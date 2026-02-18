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

}