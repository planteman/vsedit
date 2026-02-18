use std::fmt;
use std::sync::Arc;

use vsedit_editor_config::WordWrap;
use vsedit_editor_types::{ITextModel, Position, Range};
use vsedit_text_model::TextModel;

/// A single display line after word wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewLine {
    pub content: String,
    /// 1-based model line this view line belongs to.
    pub model_line: u32,
    /// 1-based column where this view line starts in the model line.
    pub model_start_column: u32,
    /// `true` if this is a continuation of a wrapped line.
    pub is_wrapped: bool,
}

/// Maps text model lines to display lines with optional word wrapping.
pub struct ViewModel {
    model: Arc<TextModel>,
    view_lines: Vec<ViewLine>,
    /// Characters per line before wrapping. 0 means no wrapping.
    wrap_width: u32,
    word_wrap: WordWrap,
}

impl ViewModel {
    pub fn new(model: Arc<TextModel>, wrap_width: u32, word_wrap: WordWrap) -> Self {
        let mut vm = Self {
            model,
            view_lines: Vec::new(),
            wrap_width,
            word_wrap,
        };
        vm.recompute();
        vm
    }

    /// Recalculate all view lines from the underlying model.
    pub fn recompute(&mut self) {
        self.view_lines.clear();
        let effective_width = self.effective_wrap_width();
        let line_count = self.model.get_line_count();

        for model_line in 1..=line_count {
            let content = self.model.get_line_content(model_line).to_string();

            if effective_width == 0 || content.len() <= effective_width as usize {
                self.view_lines.push(ViewLine {
                    content,
                    model_line,
                    model_start_column: 1,
                    is_wrapped: false,
                });
            } else {
                self.wrap_line(&content, model_line, effective_width);
            }
        }
    }

    /// Returns the total number of view lines.
    pub fn get_view_line_count(&self) -> u32 {
        self.view_lines.len() as u32
    }

    /// Returns the view line at the given 1-based index.
    ///
    /// # Panics
    /// Panics if `view_line` is 0 or exceeds the view line count.
    pub fn get_view_line(&self, view_line: u32) -> &ViewLine {
        &self.view_lines[(view_line - 1) as usize]
    }

    /// Convert a 1-based model position to a 1-based view position.
    pub fn model_position_to_view_position(&self, pos: Position) -> Position {
        for (i, vl) in self.view_lines.iter().enumerate() {
            if vl.model_line == pos.line {
                let end_col = vl.model_start_column + vl.content.len() as u32;
                if pos.column < end_col || self.is_last_view_line_for_model(i) {
                    let view_col = pos.column - vl.model_start_column + 1;
                    return Position::new((i + 1) as u32, view_col);
                }
            }
        }
        // Fallback: clamp to last view line
        let last = self.view_lines.len() as u32;
        Position::new(last, 1)
    }

    /// Convert a 1-based view position to a 1-based model position.
    pub fn view_position_to_model_position(&self, pos: Position) -> Position {
        let vl = self.get_view_line(pos.line);
        let model_col = vl.model_start_column + pos.column - 1;
        Position::new(vl.model_line, model_col)
    }

    /// Returns a slice of view lines for the given viewport.
    ///
    /// `start_view_line` is 1-based. Returns up to `count` lines.
    pub fn get_viewport_lines(&self, start_view_line: u32, count: u32) -> &[ViewLine] {
        let start = (start_view_line - 1) as usize;
        let end = (start + count as usize).min(self.view_lines.len());
        &self.view_lines[start..end]
    }

    /// Update the wrap width and recompute view lines.
    pub fn set_wrap_width(&mut self, width: u32) {
        self.wrap_width = width;
        self.recompute();
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    fn effective_wrap_width(&self) -> u32 {
        match self.word_wrap {
            WordWrap::Off | WordWrap::Inherit => 0,
            WordWrap::On | WordWrap::Bounded => self.wrap_width,
            WordWrap::WordWrapColumn => {
                if self.wrap_width > 0 {
                    self.wrap_width
                } else {
                    0
                }
            }
        }
    }

    /// Break `content` into multiple view lines at word boundaries.
    fn wrap_line(&mut self, content: &str, model_line: u32, width: u32) {
        let width = width as usize;
        let mut start = 0;
        let bytes = content.as_bytes();
        let len = bytes.len();
        let mut first = true;

        while start < len {
            let remaining = len - start;
            if remaining <= width {
                self.view_lines.push(ViewLine {
                    content: content[start..].to_string(),
                    model_line,
                    model_start_column: (start + 1) as u32,
                    is_wrapped: !first,
                });
                break;
            }

            // Find the last word-boundary position within the allowed width.
            let end = start + width;
            let mut break_at = None;
            for i in (start..end).rev() {
                if bytes[i] == b' ' || bytes[i] == b'\t' {
                    break_at = Some(i + 1);
                    break;
                }
            }
            // If no boundary found, hard-break at width.
            let break_at = break_at.unwrap_or(end);

            self.view_lines.push(ViewLine {
                content: content[start..break_at].to_string(),
                model_line,
                model_start_column: (start + 1) as u32,
                is_wrapped: !first,
            });

            start = break_at;
            first = false;
        }
    }

    /// Returns `true` if `idx` is the last view line for its model line.
    fn is_last_view_line_for_model(&self, idx: usize) -> bool {
        let model_line = self.view_lines[idx].model_line;
        match self.view_lines.get(idx + 1) {
            Some(next) => next.model_line != model_line,
            None => true,
        }
    }
}

/// Statistics about the view model state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModelStats {
    pub model_line_count: u32,
    pub view_line_count: u32,
    pub wrapped_line_count: u32,
    pub longest_view_line: u32,
}

impl std::fmt::Display for ViewModelStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "model_lines={}, view_lines={}, wrapped={}, longest={}",
            self.model_line_count,
            self.view_line_count,
            self.wrapped_line_count,
            self.longest_view_line,
        )
    }
}

impl ViewModel {
    /// Compute statistics about the current view model state.
    pub fn stats(&self) -> ViewModelStats {
        let wrapped_count = self.view_lines.iter().filter(|vl| vl.is_wrapped).count() as u32;
        let longest = self
            .view_lines
            .iter()
            .map(|vl| vl.content.len() as u32)
            .max()
            .unwrap_or(0);
        ViewModelStats {
            model_line_count: self.model.get_line_count(),
            view_line_count: self.view_lines.len() as u32,
            wrapped_line_count: wrapped_count,
            longest_view_line: longest,
        }
    }

    /// Return the current wrap width setting.
    pub fn wrap_width(&self) -> u32 {
        self.wrap_width
    }

    /// Return the current word wrap mode.
    pub fn word_wrap_mode(&self) -> WordWrap {
        self.word_wrap
    }

    /// Update the word wrap mode and recompute.
    pub fn set_word_wrap(&mut self, mode: WordWrap) {
        self.word_wrap = mode;
        self.recompute();
    }

    /// Return true if any lines are wrapped.
    pub fn has_wrapped_lines(&self) -> bool {
        self.view_lines.iter().any(|vl| vl.is_wrapped)
    }

    /// Find all view line indices (1-based) that belong to a given model line.
    pub fn view_lines_for_model_line(&self, model_line: u32) -> Vec<u32> {
        self.view_lines
            .iter()
            .enumerate()
            .filter(|(_, vl)| vl.model_line == model_line)
            .map(|(i, _)| (i + 1) as u32)
            .collect()
    }

    /// Return how many view lines a specific model line spans.
    pub fn view_line_span(&self, model_line: u32) -> u32 {
        self.view_lines
            .iter()
            .filter(|vl| vl.model_line == model_line)
            .count() as u32
    }

    /// Find the first view line index (1-based) for a given model line.
    pub fn first_view_line_for_model(&self, model_line: u32) -> Option<u32> {
        self.view_lines
            .iter()
            .position(|vl| vl.model_line == model_line)
            .map(|i| (i + 1) as u32)
    }

    /// Find the last view line index (1-based) for a given model line.
    pub fn last_view_line_for_model(&self, model_line: u32) -> Option<u32> {
        self.view_lines
            .iter()
            .enumerate()
            .rev()
            .find(|(_, vl)| vl.model_line == model_line)
            .map(|(i, _)| (i + 1) as u32)
    }

    /// Return all unique model lines that appear in the viewport.
    pub fn model_lines_in_viewport(&self, start_view_line: u32, count: u32) -> Vec<u32> {
        let lines = self.get_viewport_lines(start_view_line, count);
        let mut model_lines: Vec<u32> = lines.iter().map(|vl| vl.model_line).collect();
        model_lines.dedup();
        model_lines
    }

    /// Clamp a view position to be within valid bounds.
    pub fn clamp_view_position(&self, pos: Position) -> Position {
        let max_line = self.get_view_line_count().max(1);
        let line = pos.line.clamp(1, max_line);
        let vl = self.get_view_line(line);
        let max_col = (vl.content.len() as u32).max(1);
        let col = pos.column.clamp(1, max_col);
        Position::new(line, col)
    }

    /// Return the content of a view line at a 1-based index.
    pub fn get_view_line_content(&self, view_line: u32) -> &str {
        &self.get_view_line(view_line).content
    }

    /// Check if a view line index is within bounds.
    pub fn is_valid_view_line(&self, view_line: u32) -> bool {
        view_line >= 1 && view_line <= self.get_view_line_count()
    }
}

impl ViewLine {
    /// Return the length of the content in characters.
    pub fn char_len(&self) -> usize {
        self.content.len()
    }

    /// Return true if this line has no content.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Return the ending column (1-based, exclusive) in the model.
    pub fn model_end_column(&self) -> u32 {
        self.model_start_column + self.content.len() as u32
    }
}

impl std::fmt::Display for ViewLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_wrapped {
            write!(f, "  ↪ {}", self.content)
        } else {
            write!(f, "{:>3}│{}", self.model_line, self.content)
        }
    }
}

// ---------------------------------------------------------------------------
// Viewport calculation, visible range, coordinate mapping, line heights
// ---------------------------------------------------------------------------

/// Describes a rectangular viewport within the view model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewport {
    pub first_view_line: u32,
    pub visible_line_count: u32,
}

impl Viewport {
    pub fn new(first_view_line: u32, visible_line_count: u32) -> Self {
        Self {
            first_view_line,
            visible_line_count,
        }
    }

    /// Return the last visible view line (1-based, inclusive).
    pub fn last_view_line(&self) -> u32 {
        self.first_view_line + self.visible_line_count.saturating_sub(1)
    }

    /// Check if a 1-based view line is within this viewport.
    pub fn contains_view_line(&self, view_line: u32) -> bool {
        view_line >= self.first_view_line && view_line <= self.last_view_line()
    }
}

impl fmt::Display for Viewport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Viewport(first={}, count={})",
            self.first_view_line, self.visible_line_count
        )
    }
}

/// Tracks per-line heights for variable-height rendering.
#[derive(Debug, Clone)]
pub struct LineHeightTracker {
    heights: Vec<u32>,
    default_height: u32,
}

impl LineHeightTracker {
    pub fn new(default_height: u32) -> Self {
        Self {
            heights: Vec::new(),
            default_height: if default_height == 0 { 1 } else { default_height },
        }
    }

    /// Set the height of a specific 0-based line index.
    pub fn set_height(&mut self, line_index: usize, height: u32) {
        if line_index >= self.heights.len() {
            self.heights.resize(line_index + 1, self.default_height);
        }
        self.heights[line_index] = height;
    }

    /// Get the height of a specific 0-based line index.
    pub fn get_height(&self, line_index: usize) -> u32 {
        self.heights.get(line_index).copied().unwrap_or(self.default_height)
    }

    /// Compute the total pixel height of all tracked lines.
    pub fn total_height(&self, line_count: usize) -> u32 {
        let mut total: u32 = 0;
        for i in 0..line_count {
            total += self.get_height(i);
        }
        total
    }

    /// Find which line a given y-pixel offset falls on (0-based line index).
    pub fn line_at_offset(&self, y_offset: u32, line_count: usize) -> usize {
        let mut accumulated: u32 = 0;
        for i in 0..line_count {
            let h = self.get_height(i);
            if accumulated + h > y_offset {
                return i;
            }
            accumulated += h;
        }
        line_count.saturating_sub(1)
    }

    /// Compute the y-pixel offset of a given 0-based line index.
    pub fn offset_of_line(&self, line_index: usize) -> u32 {
        let mut offset: u32 = 0;
        for i in 0..line_index {
            offset += self.get_height(i);
        }
        offset
    }
}

impl ViewModel {
    /// Compute the visible range of model lines for a given viewport.
    pub fn visible_model_range(&self, viewport: &Viewport) -> (u32, u32) {
        let lines = self.get_viewport_lines(viewport.first_view_line, viewport.visible_line_count);
        if lines.is_empty() {
            return (0, 0);
        }
        let first_model = lines.first().unwrap().model_line;
        let last_model = lines.last().unwrap().model_line;
        (first_model, last_model)
    }

    /// Map a pixel y-coordinate to a view line (1-based) using a line height tracker.
    pub fn view_line_at_pixel(&self, y: u32, tracker: &LineHeightTracker) -> u32 {
        let count = self.get_view_line_count() as usize;
        let idx = tracker.line_at_offset(y, count);
        (idx as u32) + 1
    }

    /// Compute the pixel y-offset of a view line (1-based) using a line height tracker.
    pub fn pixel_offset_of_view_line(&self, view_line: u32, tracker: &LineHeightTracker) -> u32 {
        tracker.offset_of_line((view_line - 1) as usize)
    }
}

/// A search match within the view model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModelSearchMatch {
    /// 1-based view line number.
    pub view_line: u32,
    /// 0-based start column in the view line content.
    pub start_col: u32,
    /// 0-based end column (exclusive) in the view line content.
    pub end_col: u32,
}

/// Searches for a pattern across view lines in the view model.
pub struct ViewModelSearch;

impl ViewModelSearch {
    /// Find all occurrences of `pattern` in the view model's view lines.
    pub fn find(vm: &ViewModel, pattern: &str) -> Vec<ViewModelSearchMatch> {
        let mut results = Vec::new();
        if pattern.is_empty() {
            return results;
        }
        let count = vm.get_view_line_count();
        for i in 1..=count {
            let content = vm.get_view_line_content(i);
            let mut start = 0;
            while let Some(pos) = content[start..].find(pattern) {
                let abs_start = start + pos;
                results.push(ViewModelSearchMatch {
                    view_line: i,
                    start_col: abs_start as u32,
                    end_col: (abs_start + pattern.len()) as u32,
                });
                start = abs_start + 1;
            }
        }
        results
    }
}

// ---------------------------------------------------------------------------
// VisibleRange — tracks the range of visible lines
// ---------------------------------------------------------------------------

/// Tracks the first and last visible line in the editor viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRange {
    /// 1-based first visible view line.
    pub first_line: u32,
    /// 1-based last visible view line (inclusive).
    pub last_line: u32,
}

impl VisibleRange {
    pub fn new(first_line: u32, last_line: u32) -> Self {
        Self { first_line, last_line }
    }

    /// Number of visible lines.
    pub fn line_count(&self) -> u32 {
        if self.last_line >= self.first_line {
            self.last_line - self.first_line + 1
        } else {
            0
        }
    }

    /// Check if a view line (1-based) is within this visible range.
    pub fn contains(&self, view_line: u32) -> bool {
        view_line >= self.first_line && view_line <= self.last_line
    }

    /// Compute the overlap between two visible ranges.
    pub fn overlap(&self, other: &VisibleRange) -> Option<VisibleRange> {
        let first = self.first_line.max(other.first_line);
        let last = self.last_line.min(other.last_line);
        if first <= last {
            Some(VisibleRange::new(first, last))
        } else {
            None
        }
    }

    /// Returns true if the range is empty (last < first).
    pub fn is_empty(&self) -> bool {
        self.last_line < self.first_line
    }
}

impl fmt::Display for VisibleRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VisibleRange({}-{})", self.first_line, self.last_line)
    }
}

impl ViewModel {
    /// Compute the [`VisibleRange`] for a given viewport.
    pub fn visible_range(&self, viewport: &Viewport) -> VisibleRange {
        let total = self.get_view_line_count();
        let first = viewport.first_view_line.min(total).max(1);
        let last = (viewport.first_view_line + viewport.visible_line_count - 1).min(total);
        VisibleRange::new(first, last)
    }

    /// Map a visible range of view lines to the corresponding model line range.
    pub fn visible_range_to_model(&self, range: &VisibleRange) -> (u32, u32) {
        if range.is_empty() || self.view_lines.is_empty() {
            return (0, 0);
        }
        let first_model = self.get_view_line(range.first_line).model_line;
        let last_model = self.get_view_line(range.last_line).model_line;
        (first_model, last_model)
    }

    /// Compute the scroll offset needed to reveal a target view line within a viewport.
    ///
    /// Returns the new first visible view line such that `target_view_line` is within
    /// the viewport. If the line is already visible, returns `None`.
    pub fn scroll_to_reveal(&self, target_view_line: u32, viewport: &Viewport) -> Option<u32> {
        let total = self.get_view_line_count();
        if target_view_line < 1 || target_view_line > total {
            return None;
        }

        let first = viewport.first_view_line;
        let last = viewport.first_view_line + viewport.visible_line_count.saturating_sub(1);

        if target_view_line >= first && target_view_line <= last {
            // Already visible
            return None;
        }

        if target_view_line < first {
            // Scroll up: target becomes the first line
            Some(target_view_line)
        } else {
            // Scroll down: target becomes the last line
            Some(target_view_line.saturating_sub(viewport.visible_line_count.saturating_sub(1)))
        }
    }

    /// Compute the scroll offset to center a target view line in the viewport.
    pub fn scroll_to_center(&self, target_view_line: u32, viewport_height: u32) -> u32 {
        let total = self.get_view_line_count();
        if target_view_line < 1 || target_view_line > total || viewport_height == 0 {
            return 1;
        }
        let half = viewport_height / 2;
        let first = target_view_line.saturating_sub(half).max(1);
        let max_first = total.saturating_sub(viewport_height).max(0) + 1;
        first.min(max_first)
    }

    /// Map a model line and column to view coordinates, accounting for word wrap.
    /// Returns `(view_line, view_column)` both 1-based.
    pub fn map_model_to_view_coords(&self, model_line: u32, model_col: u32) -> (u32, u32) {
        let pos = self.model_position_to_view_position(Position::new(model_line, model_col));
        (pos.line, pos.column)
    }

    /// Map view coordinates to model coordinates, accounting for word wrap.
    /// Returns `(model_line, model_column)` both 1-based.
    pub fn map_view_to_model_coords(&self, view_line: u32, view_col: u32) -> (u32, u32) {
        let pos = self.view_position_to_model_position(Position::new(view_line, view_col));
        (pos.line, pos.column)
    }

    /// Count the total number of wrapped continuation lines in the view model.
    pub fn wrapped_line_count(&self) -> u32 {
        self.view_lines.iter().filter(|vl| vl.is_wrapped).count() as u32
    }

    /// Get the range of view lines for a model line range.
    /// Returns (first_view_line, last_view_line) both 1-based.
    pub fn view_line_range_for_model_range(
        &self,
        first_model: u32,
        last_model: u32,
    ) -> Option<(u32, u32)> {
        let first_vl = self.first_view_line_for_model(first_model)?;
        let last_vl = self.last_view_line_for_model(last_model)?;
        Some((first_vl, last_vl))
    }
}

// ---------------------------------------------------------------------------
// Viewport – scroll helpers
// ---------------------------------------------------------------------------

impl Viewport {
    /// Return a new viewport scrolled down by `lines`.
    pub fn scroll_down(&self, lines: u32) -> Viewport {
        Viewport {
            first_view_line: self.first_view_line.saturating_add(lines),
            visible_line_count: self.visible_line_count,
        }
    }

    /// Return a new viewport scrolled up by `lines`.
    pub fn scroll_up(&self, lines: u32) -> Viewport {
        Viewport {
            first_view_line: self.first_view_line.saturating_sub(lines).max(1),
            visible_line_count: self.visible_line_count,
        }
    }

    /// Returns `true` if the viewport shows zero lines.
    pub fn is_empty(&self) -> bool {
        self.visible_line_count == 0
    }
}

// ---------------------------------------------------------------------------
// VisibleRange – merge
// ---------------------------------------------------------------------------

impl VisibleRange {
    /// Merge two ranges into one that spans both.
    pub fn merge(&self, other: &VisibleRange) -> VisibleRange {
        VisibleRange {
            first_line: self.first_line.min(other.first_line),
            last_line: self.last_line.max(other.last_line),
        }
    }
}

// ---------------------------------------------------------------------------
// ViewLine – helpers
// ---------------------------------------------------------------------------

impl ViewLine {
    /// Returns `true` if this view line is a continuation of a wrapped line.
    pub fn is_continuation(&self) -> bool {
        self.is_wrapped
    }
}

// ---------------------------------------------------------------------------
// LineHeightTracker – helpers
// ---------------------------------------------------------------------------

impl LineHeightTracker {
    /// Compute the average line height given a total line count.
    pub fn average_height(&self, line_count: usize) -> f64 {
        if line_count == 0 {
            return self.default_height as f64;
        }
        self.total_height(line_count) as f64 / line_count as f64
    }

    /// Returns `true` if any line has a custom (non-default) height.
    pub fn has_custom_heights(&self) -> bool {
        self.heights.iter().any(|&h| h != self.default_height)
    }
}

// ---------------------------------------------------------------------------
// ViewModelStats – helpers
// ---------------------------------------------------------------------------

impl ViewModelStats {
    /// Return a human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "{} model lines, {} view lines ({} wrapped), longest={}",
            self.model_line_count,
            self.view_line_count,
            self.wrapped_line_count,
            self.longest_view_line,
        )
    }
}

// ---------------------------------------------------------------------------
// ViewLineRange — a range of 1-based view lines
// ---------------------------------------------------------------------------

/// A range of 1-based view line indices (inclusive on both ends).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewLineRange {
    /// First view line (1-based, inclusive).
    pub start: u32,
    /// Last view line (1-based, inclusive).
    pub end: u32,
}

impl ViewLineRange {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Returns `true` if `view_line` is within this range.
    pub fn contains(&self, view_line: u32) -> bool {
        !self.is_empty() && view_line >= self.start && view_line <= self.end
    }

    /// Number of view lines in the range.
    pub fn len(&self) -> u32 {
        if self.end >= self.start {
            self.end - self.start + 1
        } else {
            0
        }
    }

    /// Returns `true` if the range contains no lines.
    pub fn is_empty(&self) -> bool {
        self.end < self.start
    }

    /// Compute the intersection with another range, or `None` if disjoint.
    pub fn intersect(&self, other: &ViewLineRange) -> Option<ViewLineRange> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if start <= end {
            Some(ViewLineRange::new(start, end))
        } else {
            None
        }
    }

    /// Compute the smallest range that covers both ranges.
    pub fn union(&self, other: &ViewLineRange) -> ViewLineRange {
        ViewLineRange::new(self.start.min(other.start), self.end.max(other.end))
    }
}

impl fmt::Display for ViewLineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{}]", self.start, self.end)
    }
}

impl From<(u32, u32)> for ViewLineRange {
    fn from((start, end): (u32, u32)) -> Self {
        Self::new(start, end)
    }
}

impl From<&VisibleRange> for ViewLineRange {
    fn from(vr: &VisibleRange) -> Self {
        Self::new(vr.first_line, vr.last_line)
    }
}

// ---------------------------------------------------------------------------
// ViewportState — tracks scroll position, cursor, and visible range
// ---------------------------------------------------------------------------

/// Mutable state for a viewport: scroll position, visible height, and cursor line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportState {
    /// 1-based first visible view line.
    pub scroll_position: u32,
    /// Number of lines visible in the viewport.
    pub viewport_height: u32,
    /// 1-based view line where the cursor currently sits.
    pub cursor_view_line: u32,
}

impl ViewportState {
    pub fn new(viewport_height: u32) -> Self {
        Self {
            scroll_position: 1,
            viewport_height,
            cursor_view_line: 1,
        }
    }

    /// Scroll so that `target` is the first visible line, clamped to valid bounds.
    pub fn scroll_to(&mut self, target: u32, total_view_lines: u32) {
        let max_first = total_view_lines.saturating_sub(self.viewport_height) + 1;
        self.scroll_position = target.clamp(1, max_first);
    }

    /// Adjust scroll position so that `cursor_view_line` is visible.
    pub fn ensure_visible(&mut self, total_view_lines: u32) {
        if self.cursor_view_line < self.scroll_position {
            self.scroll_to(self.cursor_view_line, total_view_lines);
        } else if self.cursor_view_line
            > self.scroll_position + self.viewport_height.saturating_sub(1)
        {
            let new_first = self.cursor_view_line.saturating_sub(self.viewport_height - 1);
            self.scroll_to(new_first, total_view_lines);
        }
    }

    /// Returns `true` if `view_line` is currently visible.
    pub fn is_line_visible(&self, view_line: u32) -> bool {
        view_line >= self.scroll_position
            && view_line <= self.scroll_position + self.viewport_height.saturating_sub(1)
    }

    /// Return the currently visible range as a `ViewLineRange`.
    pub fn visible_range(&self, total_view_lines: u32) -> ViewLineRange {
        let end = (self.scroll_position + self.viewport_height - 1).min(total_view_lines);
        ViewLineRange::new(self.scroll_position, end)
    }
}

impl fmt::Display for ViewportState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ViewportState(scroll={}, height={}, cursor={})",
            self.scroll_position, self.viewport_height, self.cursor_view_line
        )
    }
}

// ---------------------------------------------------------------------------
// ViewLineIterator — iterate over view lines with optional filtering
// ---------------------------------------------------------------------------

/// An iterator over `(1-based index, &ViewLine)` pairs with optional filters.
pub struct ViewLineIterator<'a> {
    view_lines: &'a [ViewLine],
    pos: usize,
    wrapped_only: bool,
    model_line_min: Option<u32>,
    model_line_max: Option<u32>,
}

impl<'a> ViewLineIterator<'a> {
    pub fn new(vm: &'a ViewModel) -> Self {
        Self {
            view_lines: &vm.view_lines,
            pos: 0,
            wrapped_only: false,
            model_line_min: None,
            model_line_max: None,
        }
    }

    /// Only yield wrapped continuation lines.
    pub fn wrapped_only(mut self) -> Self {
        self.wrapped_only = true;
        self
    }

    /// Only yield lines from model lines in `[min_model, max_model]` (inclusive).
    pub fn model_line_range(mut self, min_model: u32, max_model: u32) -> Self {
        self.model_line_min = Some(min_model);
        self.model_line_max = Some(max_model);
        self
    }
}

impl<'a> Iterator for ViewLineIterator<'a> {
    type Item = (u32, &'a ViewLine);

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.view_lines.len() {
            let idx = self.pos;
            self.pos += 1;
            let vl = &self.view_lines[idx];

            if self.wrapped_only && !vl.is_wrapped {
                continue;
            }
            if let Some(min) = self.model_line_min {
                if vl.model_line < min {
                    continue;
                }
            }
            if let Some(max) = self.model_line_max {
                if vl.model_line > max {
                    continue;
                }
            }
            return Some(((idx as u32) + 1, vl));
        }
        None
    }
}

// ---------------------------------------------------------------------------
// ViewLine – content analysis helpers
// ---------------------------------------------------------------------------

impl ViewLine {
    /// Return the leading whitespace count (spaces and tabs).
    pub fn leading_whitespace(&self) -> u32 {
        self.content
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count() as u32
    }

    /// Return the trimmed (leading + trailing whitespace removed) content.
    pub fn trimmed_content(&self) -> &str {
        self.content.trim()
    }

    /// Return `true` if the view line consists entirely of whitespace.
    pub fn is_whitespace_only(&self) -> bool {
        self.content.bytes().all(|b| b == b' ' || b == b'\t')
    }

    /// Return the model range covered by this view line as a `Range`.
    pub fn model_range(&self) -> Range {
        Range::new(
            self.model_line,
            self.model_start_column,
            self.model_line,
            self.model_end_column(),
        )
    }
}

// ---------------------------------------------------------------------------
// ViewLineRange – iteration helpers
// ---------------------------------------------------------------------------

impl ViewLineRange {
    /// Iterate over all 1-based line indices in the range.
    pub fn iter(&self) -> impl Iterator<Item = u32> {
        let start = self.start;
        let end = if self.is_empty() {
            self.start // produce empty iterator
        } else {
            self.end + 1
        };
        start..end
    }

    /// Clamp the range to `[1, max_line]`.
    pub fn clamp(&self, max_line: u32) -> ViewLineRange {
        if self.is_empty() || max_line == 0 {
            return ViewLineRange::new(1, 0); // empty
        }
        ViewLineRange::new(self.start.max(1).min(max_line), self.end.min(max_line))
    }

    /// Expand the range by `amount` lines on each side, staying within `[1, max_line]`.
    pub fn expand(&self, amount: u32, max_line: u32) -> ViewLineRange {
        if self.is_empty() {
            return self.clone();
        }
        let new_start = self.start.saturating_sub(amount).max(1);
        let new_end = self.end.saturating_add(amount).min(max_line);
        ViewLineRange::new(new_start, new_end)
    }
}

// ---------------------------------------------------------------------------
// ViewModel – content search & range queries
// ---------------------------------------------------------------------------

impl ViewModel {
    /// Return the model `Range` that a given 1-based view line covers.
    pub fn view_line_model_range(&self, view_line: u32) -> Range {
        self.get_view_line(view_line).model_range()
    }

    /// Collect all view lines whose content contains `needle` (case-sensitive).
    /// Returns 1-based view line indices.
    pub fn find_lines_containing(&self, needle: &str) -> Vec<u32> {
        if needle.is_empty() {
            return Vec::new();
        }
        (1..=self.get_view_line_count())
            .filter(|&i| self.get_view_line_content(i).contains(needle))
            .collect()
    }

    /// Collect all view lines whose trimmed content matches `text` exactly.
    /// Returns 1-based view line indices.
    pub fn find_exact_trimmed(&self, text: &str) -> Vec<u32> {
        (1..=self.get_view_line_count())
            .filter(|&i| self.get_view_line(i).trimmed_content() == text)
            .collect()
    }

    /// Return all view lines that are blank (empty or whitespace-only).
    /// Returns 1-based view line indices.
    pub fn blank_lines(&self) -> Vec<u32> {
        (1..=self.get_view_line_count())
            .filter(|&i| {
                let vl = self.get_view_line(i);
                vl.is_empty() || vl.is_whitespace_only()
            })
            .collect()
    }

    /// Compute the indentation level (leading whitespace chars) for each view line.
    /// Returns a `Vec` indexed by 0-based view line index.
    pub fn indentation_map(&self) -> Vec<u32> {
        (1..=self.get_view_line_count())
            .map(|i| self.get_view_line(i).leading_whitespace())
            .collect()
    }

    /// Return the maximum column used across all view lines (the "virtual width").
    pub fn max_column(&self) -> u32 {
        self.view_lines
            .iter()
            .map(|vl| vl.content.len() as u32)
            .max()
            .unwrap_or(0)
    }

    /// Return the total character count across all view lines.
    pub fn total_character_count(&self) -> usize {
        self.view_lines.iter().map(|vl| vl.content.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// ViewportState – cursor movement helpers
// ---------------------------------------------------------------------------

impl ViewportState {
    /// Move cursor up by `n` lines, scrolling if needed. Returns the new cursor line.
    pub fn move_cursor_up(&mut self, n: u32, total_view_lines: u32) -> u32 {
        self.cursor_view_line = self.cursor_view_line.saturating_sub(n).max(1);
        self.ensure_visible(total_view_lines);
        self.cursor_view_line
    }

    /// Move cursor down by `n` lines, scrolling if needed. Returns the new cursor line.
    pub fn move_cursor_down(&mut self, n: u32, total_view_lines: u32) -> u32 {
        self.cursor_view_line = self
            .cursor_view_line
            .saturating_add(n)
            .min(total_view_lines);
        self.ensure_visible(total_view_lines);
        self.cursor_view_line
    }

    /// Page down: move cursor by viewport height and scroll accordingly.
    pub fn page_down(&mut self, total_view_lines: u32) {
        self.move_cursor_down(self.viewport_height, total_view_lines);
    }

    /// Page up: move cursor by viewport height and scroll accordingly.
    pub fn page_up(&mut self, total_view_lines: u32) {
        self.move_cursor_up(self.viewport_height, total_view_lines);
    }

    /// Move cursor to the very first line.
    pub fn go_to_top(&mut self, total_view_lines: u32) {
        self.cursor_view_line = 1;
        self.ensure_visible(total_view_lines);
    }

    /// Move cursor to the very last line.
    pub fn go_to_bottom(&mut self, total_view_lines: u32) {
        self.cursor_view_line = total_view_lines.max(1);
        self.ensure_visible(total_view_lines);
    }

    /// Return the 0-based "progress" through the document as a fraction `[0.0, 1.0]`.
    pub fn scroll_fraction(&self, total_view_lines: u32) -> f64 {
        if total_view_lines <= self.viewport_height {
            return 0.0;
        }
        let max_scroll = total_view_lines - self.viewport_height;
        (self.scroll_position - 1) as f64 / max_scroll as f64
    }
}

// ---------------------------------------------------------------------------
// Viewport – range & position helpers
// ---------------------------------------------------------------------------

impl Viewport {
    /// Clamp this viewport so it does not exceed `total_view_lines`.
    pub fn clamp(&self, total_view_lines: u32) -> Viewport {
        let first = self.first_view_line.max(1).min(total_view_lines.max(1));
        let max_count = total_view_lines.saturating_sub(first - 1);
        Viewport::new(first, self.visible_line_count.min(max_count))
    }

    /// Return the midpoint view line of this viewport.
    pub fn center_line(&self) -> u32 {
        self.first_view_line + self.visible_line_count / 2
    }

    /// Convert this viewport to a `ViewLineRange`.
    pub fn to_range(&self) -> ViewLineRange {
        ViewLineRange::new(self.first_view_line, self.last_view_line())
    }
}

// ---------------------------------------------------------------------------
// LineHeightTracker – reset & resize
// ---------------------------------------------------------------------------

impl LineHeightTracker {
    /// Reset all custom heights back to the default.
    pub fn reset(&mut self) {
        self.heights.clear();
    }

    /// Return the default line height.
    pub fn default_height(&self) -> u32 {
        self.default_height
    }

    /// Number of lines that have explicit height entries.
    pub fn explicit_count(&self) -> usize {
        self.heights.len()
    }

    /// Find the line index with the maximum height (0-based). Returns `None` if empty.
    pub fn tallest_line(&self, line_count: usize) -> Option<usize> {
        if line_count == 0 {
            return None;
        }
        let mut max_h = 0u32;
        let mut max_idx = 0usize;
        for i in 0..line_count {
            let h = self.get_height(i);
            if h >= max_h {
                max_h = h;
                max_idx = i;
            }
        }
        Some(max_idx)
    }
}


// ---------------------------------------------------------------------------
// ViewModelColumnCache – cached column widths for wrapped lines
// ---------------------------------------------------------------------------

/// Caches the column offset for each view line to avoid recomputation during
/// scrolling or cursor movement.
#[derive(Debug, Clone)]
pub struct ViewModelColumnCache {
    /// One entry per view line: the cumulative column offset within the model line.
    offsets: Vec<u32>,
    /// Whether the cache is valid.
    valid: bool,
}

impl ViewModelColumnCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            offsets: Vec::new(),
            valid: false,
        }
    }

    /// Build (or rebuild) the cache from the current set of view lines.
    pub fn rebuild(&mut self, view_lines: &[ViewLine]) {
        self.offsets.clear();
        self.offsets.reserve(view_lines.len());
        for vl in view_lines {
            self.offsets.push(vl.model_start_column.saturating_sub(1));
        }
        self.valid = true;
    }

    /// Returns `true` if the cache has been built and has not been invalidated.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Invalidate the cache so it will be rebuilt on the next access.
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Look up the column offset for a specific view line index (0-based).
    /// Returns `None` if the cache is invalid or `idx` is out of range.
    pub fn get_offset(&self, idx: usize) -> Option<u32> {
        if !self.valid {
            return None;
        }
        self.offsets.get(idx).copied()
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Returns the maximum column offset across all view lines, or 0 if empty.
    pub fn max_offset(&self) -> u32 {
        self.offsets.iter().copied().max().unwrap_or(0)
    }

    /// Returns the sum of all column offsets (useful for statistics / debugging).
    pub fn total_offset(&self) -> u64 {
        self.offsets.iter().map(|&o| o as u64).sum()
    }
}

impl Default for ViewModelColumnCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ViewModelScrollDelta – scroll position tracking
// ---------------------------------------------------------------------------

/// Tracks a delta between two scroll positions expressed in view lines and
/// fractional columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewModelScrollDelta {
    /// Number of view lines to scroll (positive = down, negative = up).
    pub delta_lines: i64,
    /// Fractional column offset (0.0–1.0 of a character width).
    pub delta_columns: f64,
}

impl ViewModelScrollDelta {
    /// Zero delta (no scroll).
    pub fn zero() -> Self {
        Self {
            delta_lines: 0,
            delta_columns: 0.0,
        }
    }

    /// Create a delta from line and column counts.
    pub fn new(delta_lines: i64, delta_columns: f64) -> Self {
        Self {
            delta_lines,
            delta_columns,
        }
    }

    /// Whether this delta is effectively zero.
    pub fn is_zero(&self) -> bool {
        self.delta_lines == 0 && self.delta_columns.abs() < f64::EPSILON
    }

    /// Negate the delta (reverse direction).
    pub fn negate(&self) -> Self {
        Self {
            delta_lines: -self.delta_lines,
            delta_columns: -self.delta_columns,
        }
    }

    /// Add two deltas component-wise.
    pub fn add(&self, other: &Self) -> Self {
        Self {
            delta_lines: self.delta_lines + other.delta_lines,
            delta_columns: self.delta_columns + other.delta_columns,
        }
    }

    /// Scale the delta by an integer factor.
    pub fn scale(&self, factor: i64) -> Self {
        Self {
            delta_lines: self.delta_lines * factor,
            delta_columns: self.delta_columns * factor as f64,
        }
    }

    /// Return the absolute magnitude of the line delta.
    pub fn abs_lines(&self) -> u64 {
        self.delta_lines.unsigned_abs()
    }
}

impl fmt::Display for ViewModelScrollDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScrollΔ(lines={}, cols={:.2})", self.delta_lines, self.delta_columns)
    }
}

// ---------------------------------------------------------------------------
// View-line range calculator
// ---------------------------------------------------------------------------

/// Calculates which view lines are visible given a scroll position and
/// viewport height (both expressed in view-line counts).
#[derive(Debug, Clone)]
pub struct ViewModelLineRangeCalculator {
    /// Total number of view lines in the model.
    total_view_lines: usize,
}

impl ViewModelLineRangeCalculator {
    pub fn new(total_view_lines: usize) -> Self {
        Self { total_view_lines }
    }

    /// Return the 0-based start and end (exclusive) of visible view lines.
    pub fn visible_range(&self, scroll_top: usize, viewport_height: usize) -> (usize, usize) {
        let start = scroll_top.min(self.total_view_lines);
        let end = (start + viewport_height).min(self.total_view_lines);
        (start, end)
    }

    /// Number of lines visible in the returned range.
    pub fn visible_count(&self, scroll_top: usize, viewport_height: usize) -> usize {
        let (start, end) = self.visible_range(scroll_top, viewport_height);
        end - start
    }

    /// Whether the scroll position is at the very top.
    pub fn is_at_top(&self, scroll_top: usize) -> bool {
        scroll_top == 0
    }

    /// Whether the scroll position shows the very last line.
    pub fn is_at_bottom(&self, scroll_top: usize, viewport_height: usize) -> bool {
        scroll_top + viewport_height >= self.total_view_lines
    }

    /// Clamp a proposed scroll position to valid bounds.
    pub fn clamp_scroll(&self, scroll_top: usize, viewport_height: usize) -> usize {
        if self.total_view_lines <= viewport_height {
            return 0;
        }
        scroll_top.min(self.total_view_lines - viewport_height)
    }

    /// Return how many lines remain below the current viewport.
    pub fn lines_below(&self, scroll_top: usize, viewport_height: usize) -> usize {
        let (_, end) = self.visible_range(scroll_top, viewport_height);
        self.total_view_lines.saturating_sub(end)
    }

    /// Return how many lines are above the current viewport.
    pub fn lines_above(&self, scroll_top: usize) -> usize {
        scroll_top.min(self.total_view_lines)
    }

    /// Scroll by `delta` lines (positive = down), clamping to valid bounds.
    pub fn scroll_by(&self, scroll_top: usize, delta: i64, viewport_height: usize) -> usize {
        let new_top = if delta >= 0 {
            scroll_top.saturating_add(delta as usize)
        } else {
            scroll_top.saturating_sub(delta.unsigned_abs() as usize)
        };
        self.clamp_scroll(new_top, viewport_height)
    }
}

// ---------------------------------------------------------------------------
// Coordinate transform – model ↔ view position mapping
// ---------------------------------------------------------------------------

/// Converts between model positions (line, column) and view positions
/// (view line index, column within the view line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewModelCoordinate {
    /// 0-based view line index.
    pub view_line: usize,
    /// 1-based column within the view line.
    pub view_column: u32,
}

impl ViewModelCoordinate {
    pub fn new(view_line: usize, view_column: u32) -> Self {
        Self { view_line, view_column }
    }
}

/// Utility for mapping model positions to/from view coordinates.
pub struct ViewModelCoordinateTransform<'a> {
    view_lines: &'a [ViewLine],
}

impl<'a> ViewModelCoordinateTransform<'a> {
    pub fn new(view_lines: &'a [ViewLine]) -> Self {
        Self { view_lines }
    }

    /// Map a 1-based (model_line, model_column) to a view coordinate.
    /// Returns `None` if the model line is not found.
    pub fn model_to_view(&self, model_line: u32, model_column: u32) -> Option<ViewModelCoordinate> {
        for (idx, vl) in self.view_lines.iter().enumerate() {
            if vl.model_line != model_line {
                continue;
            }
            let line_end_col = vl.model_start_column + vl.content.len() as u32;
            if model_column >= vl.model_start_column && model_column < line_end_col {
                let view_col = model_column - vl.model_start_column + 1;
                return Some(ViewModelCoordinate::new(idx, view_col));
            }
            // If this is the last segment of the model line, clamp to end
            let is_last = self.view_lines.get(idx + 1)
                .map_or(true, |next| next.model_line != model_line);
            if is_last && model_column >= vl.model_start_column {
                let view_col = (model_column - vl.model_start_column + 1)
                    .min(vl.content.len() as u32 + 1);
                return Some(ViewModelCoordinate::new(idx, view_col));
            }
        }
        None
    }

    /// Map a view coordinate back to a (model_line, model_column) pair.
    /// Returns `None` if the view line index is out of range.
    pub fn view_to_model(&self, coord: &ViewModelCoordinate) -> Option<(u32, u32)> {
        let vl = self.view_lines.get(coord.view_line)?;
        let model_col = vl.model_start_column + coord.view_column.saturating_sub(1);
        Some((vl.model_line, model_col))
    }

    /// Return the view line index of the first view line for a given model line.
    pub fn first_view_line_for_model(&self, model_line: u32) -> Option<usize> {
        self.view_lines.iter().position(|vl| vl.model_line == model_line)
    }

    /// Return the view line index of the last view line for a given model line.
    pub fn last_view_line_for_model(&self, model_line: u32) -> Option<usize> {
        self.view_lines.iter().rposition(|vl| vl.model_line == model_line)
    }

    /// Count how many view lines correspond to a given model line.
    pub fn view_line_count_for_model(&self, model_line: u32) -> usize {
        self.view_lines.iter().filter(|vl| vl.model_line == model_line).count()
    }

    /// Total number of view lines.
    pub fn total_view_lines(&self) -> usize {
        self.view_lines.len()
    }

    /// Returns the model line numbers that have wrapped (i.e. span more than one view line).
    pub fn wrapped_model_lines(&self) -> Vec<u32> {
        let mut result = Vec::new();
        let mut last_model_line: Option<u32> = None;
        for vl in self.view_lines {
            if vl.is_wrapped {
                if last_model_line != Some(vl.model_line) {
                    result.push(vl.model_line);
                    last_model_line = Some(vl.model_line);
                }
            }
        }
        result
    }
}


// ---------------------------------------------------------------------------
// viewmodel – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XViewmodelLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XViewmodelPanelState {
    pub region: XViewmodelLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XViewmodelPanelState {
    pub fn new(region: XViewmodelLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_viewmodel_total_visible_area(panels: &[XViewmodelPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_viewmodel_count_in_region(
    panels: &[XViewmodelPanelState],
    region: XViewmodelLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_viewmodel_widest_panel(panels: &[XViewmodelPanelState]) -> Option<&XViewmodelPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_viewmodel_collapse_region(
    panels: &mut [XViewmodelPanelState],
    region: XViewmodelLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XViewmodelLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XViewmodelLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// viewmodel – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for view model binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YViewmodelViewModelChangeKind {
    Insert,
    Update,
    Delete,
    Reset,
}

impl YViewmodelViewModelChangeKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Insert => 0,
            Self::Update => 1,
            Self::Delete => 2,
            Self::Reset => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Insert => "Insert",
            Self::Update => "Update",
            Self::Delete => "Delete",
            Self::Reset => "Reset",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YViewmodelViewModelChangeKind] {
        &[
            YViewmodelViewModelChangeKind::Insert,
            YViewmodelViewModelChangeKind::Update,
            YViewmodelViewModelChangeKind::Delete,
            YViewmodelViewModelChangeKind::Reset,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YViewmodelViewModelChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks change log data.
#[derive(Debug, Clone)]
pub struct YViewmodelViewModelChangeLog {
    pub changes: Vec<(u64, String)>,
    pub version: u64,
    pub compacted: bool,
}

impl YViewmodelViewModelChangeLog {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            version: 0,
            compacted: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YViewmodelViewModelChangeLog({}: {:?})", "changes", self.changes)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_viewmodel_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_viewmodel_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_viewmodel_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_viewmodel_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_viewmodel_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_viewmodel_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_viewmodel_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_viewmodel_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// viewmodel – Extended view model snapshot helpers
// ---------------------------------------------------------------------------

/// Priority levels for view model snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZViewmodelPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZViewmodelPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZViewmodelPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZViewmodelPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks view model snapshot data.
#[derive(Debug, Clone)]
pub struct ZViewmodelViewModelSnapshot {
    pub fields: Vec<(String, String)>,
    pub version: u64,
    pub frozen: bool,
}

impl ZViewmodelViewModelSnapshot {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            version: 0,
            frozen: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZViewmodelViewModelSnapshot[version={:?}, frozen={:?}]", self.version, self.frozen)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.frozen = !c.frozen;
        c
    }
}

/// Compute a simple rolling hash for view model snapshot.
pub fn z_viewmodel_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_viewmodel_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_viewmodel_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_viewmodel_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_viewmodel_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_viewmodel_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_viewmodel_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 87
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer87 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer87 {
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
pub fn xb_fnv1a_87(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_87<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_87<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_87(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_87(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model(text: &str) -> Arc<TextModel> {
        Arc::new(TextModel::new(text))
    }

    #[test]
    fn no_wrap_identity() {
        let model = make_model("hello\nworld\nfoo");
        let vm = ViewModel::new(model, 0, WordWrap::Off);

        assert_eq!(vm.get_view_line_count(), 3);
        assert_eq!(vm.get_view_line(1).content, "hello");
        assert_eq!(vm.get_view_line(2).content, "world");
        assert_eq!(vm.get_view_line(3).content, "foo");

        for i in 1..=3 {
            let vl = vm.get_view_line(i);
            assert_eq!(vl.model_line, i);
            assert_eq!(vl.model_start_column, 1);
            assert!(!vl.is_wrapped);
        }
    }

    #[test]
    fn word_wrap_breaks_long_line() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);

        assert_eq!(vm.get_view_line_count(), 2);

        let vl1 = vm.get_view_line(1);
        assert_eq!(vl1.content, "hello ");
        assert_eq!(vl1.model_line, 1);
        assert_eq!(vl1.model_start_column, 1);
        assert!(!vl1.is_wrapped);

        let vl2 = vm.get_view_line(2);
        assert_eq!(vl2.content, "world");
        assert_eq!(vl2.model_line, 1);
        assert_eq!(vl2.model_start_column, 7);
        assert!(vl2.is_wrapped);
    }

    #[test]
    fn word_wrap_no_boundary_hard_break() {
        let model = make_model("abcdefghij");
        let vm = ViewModel::new(model, 4, WordWrap::On);

        assert_eq!(vm.get_view_line_count(), 3);
        assert_eq!(vm.get_view_line(1).content, "abcd");
        assert_eq!(vm.get_view_line(2).content, "efgh");
        assert_eq!(vm.get_view_line(3).content, "ij");
    }

    #[test]
    fn word_wrap_column_variant() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::WordWrapColumn);

        assert_eq!(vm.get_view_line_count(), 2);
        assert_eq!(vm.get_view_line(1).content, "hello ");
        assert_eq!(vm.get_view_line(2).content, "world");
    }

    #[test]
    fn model_to_view_no_wrap() {
        let model = make_model("aaa\nbbb\nccc");
        let vm = ViewModel::new(model, 0, WordWrap::Off);

        assert_eq!(
            vm.model_position_to_view_position(Position::new(2, 2)),
            Position::new(2, 2)
        );
    }

    #[test]
    fn model_to_view_with_wrap() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);

        assert_eq!(
            vm.model_position_to_view_position(Position::new(1, 1)),
            Position::new(1, 1)
        );
        // model (1,7) is 'w' in "world" → view (2,1)
        assert_eq!(
            vm.model_position_to_view_position(Position::new(1, 7)),
            Position::new(2, 1)
        );
        // model (1,9) is 'r' → view (2,3)
        assert_eq!(
            vm.model_position_to_view_position(Position::new(1, 9)),
            Position::new(2, 3)
        );
    }

    #[test]
    fn view_to_model_no_wrap() {
        let model = make_model("aaa\nbbb\nccc");
        let vm = ViewModel::new(model, 0, WordWrap::Off);

        assert_eq!(
            vm.view_position_to_model_position(Position::new(3, 2)),
            Position::new(3, 2)
        );
    }

    #[test]
    fn view_to_model_with_wrap() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);

        assert_eq!(
            vm.view_position_to_model_position(Position::new(2, 1)),
            Position::new(1, 7)
        );
    }

    #[test]
    fn viewport_lines() {
        let model = make_model("a\nb\nc\nd\ne");
        let vm = ViewModel::new(model, 0, WordWrap::Off);

        let vp = vm.get_viewport_lines(2, 3);
        assert_eq!(vp.len(), 3);
        assert_eq!(vp[0].content, "b");
        assert_eq!(vp[1].content, "c");
        assert_eq!(vp[2].content, "d");
    }

    #[test]
    fn viewport_lines_clamps_to_end() {
        let model = make_model("a\nb");
        let vm = ViewModel::new(model, 0, WordWrap::Off);

        let vp = vm.get_viewport_lines(1, 100);
        assert_eq!(vp.len(), 2);
    }

    #[test]
    fn set_wrap_width_recomputes() {
        let model = make_model("hello world");
        let mut vm = ViewModel::new(model, 0, WordWrap::On);
        assert_eq!(vm.get_view_line_count(), 1);

        vm.set_wrap_width(6);
        assert_eq!(vm.get_view_line_count(), 2);
    }

    #[test]
    fn multiple_model_lines_with_wrap() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);

        assert_eq!(vm.get_view_line_count(), 3);
        assert_eq!(vm.get_view_line(3).content, "foo");
        assert_eq!(vm.get_view_line(3).model_line, 2);
        assert!(!vm.get_view_line(3).is_wrapped);
    }

    #[test]
    fn round_trip_coordinate_conversion() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);

        let original = Position::new(1, 9);
        let view = vm.model_position_to_view_position(original);
        let back = vm.view_position_to_model_position(view);
        assert_eq!(back, original);
    }

    #[test]
    fn stats_no_wrap() {
        let model = make_model("hello\nworld\nfoo");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let stats = vm.stats();
        assert_eq!(stats.model_line_count, 3);
        assert_eq!(stats.view_line_count, 3);
        assert_eq!(stats.wrapped_line_count, 0);
        assert_eq!(stats.longest_view_line, 5);
    }

    #[test]
    fn stats_with_wrap() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let stats = vm.stats();
        assert_eq!(stats.model_line_count, 1);
        assert_eq!(stats.view_line_count, 2);
        assert_eq!(stats.wrapped_line_count, 1);
    }

    #[test]
    fn stats_display() {
        let stats = ViewModelStats {
            model_line_count: 10,
            view_line_count: 15,
            wrapped_line_count: 5,
            longest_view_line: 80,
        };
        let s = format!("{stats}");
        assert!(s.contains("model_lines=10"));
        assert!(s.contains("view_lines=15"));
    }

    #[test]
    fn wrap_width_accessor() {
        let model = make_model("hello");
        let vm = ViewModel::new(model, 42, WordWrap::Off);
        assert_eq!(vm.wrap_width(), 42);
    }

    #[test]
    fn word_wrap_mode_accessor() {
        let model = make_model("hello");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        assert_eq!(vm.word_wrap_mode(), WordWrap::Off);
    }

    #[test]
    fn set_word_wrap_recomputes() {
        let model = make_model("hello world");
        let mut vm = ViewModel::new(model, 6, WordWrap::Off);
        assert_eq!(vm.get_view_line_count(), 1);
        vm.set_word_wrap(WordWrap::On);
        assert_eq!(vm.get_view_line_count(), 2);
    }

    #[test]
    fn has_wrapped_lines_check() {
        let model = make_model("hello world");
        let vm_nowrap = ViewModel::new(model.clone(), 0, WordWrap::Off);
        assert!(!vm_nowrap.has_wrapped_lines());
        let vm_wrap = ViewModel::new(model, 6, WordWrap::On);
        assert!(vm_wrap.has_wrapped_lines());
    }

    #[test]
    fn view_lines_for_model_line_multi() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let indices = vm.view_lines_for_model_line(1);
        assert_eq!(indices, vec![1, 2]);
    }

    #[test]
    fn view_line_span_wrapped() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        assert_eq!(vm.view_line_span(1), 2);
    }

    #[test]
    fn first_and_last_view_line_for_model() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        assert_eq!(vm.first_view_line_for_model(1), Some(1));
        assert_eq!(vm.last_view_line_for_model(1), Some(2));
        assert_eq!(vm.first_view_line_for_model(2), Some(3));
        assert_eq!(vm.last_view_line_for_model(2), Some(3));
        assert_eq!(vm.first_view_line_for_model(999), None);
    }

    #[test]
    fn model_lines_in_viewport_dedup() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let mlines = vm.model_lines_in_viewport(1, 3);
        assert_eq!(mlines, vec![1, 2]);
    }

    #[test]
    fn clamp_view_position_in_bounds() {
        let model = make_model("hello\nworld");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let clamped = vm.clamp_view_position(Position::new(1, 3));
        assert_eq!(clamped, Position::new(1, 3));
    }

    #[test]
    fn clamp_view_position_out_of_bounds() {
        let model = make_model("hi\nworld");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let clamped = vm.clamp_view_position(Position::new(100, 100));
        assert_eq!(clamped.line, 2);
    }

    #[test]
    fn get_view_line_content_accessor() {
        let model = make_model("hello\nworld");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        assert_eq!(vm.get_view_line_content(1), "hello");
        assert_eq!(vm.get_view_line_content(2), "world");
    }

    #[test]
    fn is_valid_view_line_check() {
        let model = make_model("hello");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        assert!(vm.is_valid_view_line(1));
        assert!(!vm.is_valid_view_line(0));
        assert!(!vm.is_valid_view_line(2));
    }

    #[test]
    fn view_line_char_len() {
        let vl = ViewLine {
            content: "hello".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert_eq!(vl.char_len(), 5);
        assert!(!vl.is_empty());
    }

    #[test]
    fn view_line_empty() {
        let vl = ViewLine {
            content: String::new(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert!(vl.is_empty());
        assert_eq!(vl.char_len(), 0);
    }

    #[test]
    fn view_line_model_end_column() {
        let vl = ViewLine {
            content: "hello".to_string(),
            model_line: 1,
            model_start_column: 3,
            is_wrapped: true,
        };
        assert_eq!(vl.model_end_column(), 8);
    }

    #[test]
    fn view_line_display_normal() {
        let vl = ViewLine {
            content: "hello".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        let s = format!("{vl}");
        assert!(s.contains("hello"));
        assert!(s.contains("1"));
    }

    #[test]
    fn view_line_display_wrapped() {
        let vl = ViewLine {
            content: "world".to_string(),
            model_line: 1,
            model_start_column: 7,
            is_wrapped: true,
        };
        let s = format!("{vl}");
        assert!(s.contains("↪"));
        assert!(s.contains("world"));
    }

    #[test]
    fn viewport_contains_view_line() {
        let vp = Viewport::new(3, 5);
        assert_eq!(vp.last_view_line(), 7);
        assert!(!vp.contains_view_line(2));
        assert!(vp.contains_view_line(3));
        assert!(vp.contains_view_line(7));
        assert!(!vp.contains_view_line(8));
    }

    #[test]
    fn viewport_display() {
        let vp = Viewport::new(1, 10);
        let s = format!("{vp}");
        assert!(s.contains("first=1"));
        assert!(s.contains("count=10"));
    }

    #[test]
    fn line_height_tracker_basics() {
        let mut t = LineHeightTracker::new(20);
        assert_eq!(t.get_height(0), 20);
        t.set_height(0, 30);
        assert_eq!(t.get_height(0), 30);
        assert_eq!(t.total_height(3), 30 + 20 + 20);
        assert_eq!(t.offset_of_line(1), 30);
        assert_eq!(t.line_at_offset(35, 3), 1);
    }

    #[test]
    fn visible_model_range_with_wrap() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let vp = Viewport::new(1, 3);
        let (first, last) = vm.visible_model_range(&vp);
        assert_eq!(first, 1);
        assert_eq!(last, 2);
    }

    #[test]
    fn pixel_mapping_with_tracker() {
        let model = make_model("hello\nworld\nfoo");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let mut tracker = LineHeightTracker::new(20);
        tracker.set_height(0, 25);
        assert_eq!(vm.pixel_offset_of_view_line(1, &tracker), 0);
        assert_eq!(vm.pixel_offset_of_view_line(2, &tracker), 25);
        let vl = vm.view_line_at_pixel(30, &tracker);
        assert_eq!(vl, 2); // 25..45 is line 2
    }

    #[test]
    fn search_finds_pattern() {
        let model = make_model("hello world\nhello rust");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let matches = ViewModelSearch::find(&vm, "hello");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].view_line, 1);
        assert_eq!(matches[1].view_line, 2);
    }

    #[test]
    fn search_no_match() {
        let model = make_model("foo bar");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let matches = ViewModelSearch::find(&vm, "baz");
        assert!(matches.is_empty());
    }

    #[test]
    fn search_multiple_in_line() {
        let model = make_model("abab");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let matches = ViewModelSearch::find(&vm, "ab");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start_col, 0);
        assert_eq!(matches[1].start_col, 2);
    }

    // ---- VisibleRange tests ----

    #[test]
    fn visible_range_basic() {
        let vr = VisibleRange::new(3, 7);
        assert_eq!(vr.line_count(), 5);
        assert!(vr.contains(3));
        assert!(vr.contains(7));
        assert!(!vr.contains(2));
        assert!(!vr.contains(8));
        assert!(!vr.is_empty());
    }

    #[test]
    fn visible_range_overlap() {
        let a = VisibleRange::new(1, 5);
        let b = VisibleRange::new(3, 8);
        let overlap = a.overlap(&b).unwrap();
        assert_eq!(overlap.first_line, 3);
        assert_eq!(overlap.last_line, 5);

        let c = VisibleRange::new(6, 10);
        assert!(a.overlap(&c).is_none());
    }

    #[test]
    fn visible_range_empty() {
        let vr = VisibleRange::new(5, 3);
        assert!(vr.is_empty());
        assert_eq!(vr.line_count(), 0);
    }

    #[test]
    fn visible_range_display() {
        let vr = VisibleRange::new(1, 10);
        assert_eq!(format!("{vr}"), "VisibleRange(1-10)");
    }

    #[test]
    fn vm_visible_range_computation() {
        let model = make_model("a\nb\nc\nd\ne");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let vp = Viewport::new(2, 3);
        let vr = vm.visible_range(&vp);
        assert_eq!(vr.first_line, 2);
        assert_eq!(vr.last_line, 4);
        assert_eq!(vr.line_count(), 3);
    }

    #[test]
    fn vm_visible_range_to_model() {
        let model = make_model("hello world\nfoo\nbar");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        // "hello world" wraps to view lines 1,2; "foo" = 3; "bar" = 4
        let vr = VisibleRange::new(1, 3);
        let (first_m, last_m) = vm.visible_range_to_model(&vr);
        assert_eq!(first_m, 1);
        assert_eq!(last_m, 2);
    }

    // ---- scroll_to_reveal tests ----

    #[test]
    fn scroll_to_reveal_already_visible() {
        let model = make_model("a\nb\nc\nd\ne");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let vp = Viewport::new(2, 3); // lines 2-4 visible
        assert!(vm.scroll_to_reveal(3, &vp).is_none());
    }

    #[test]
    fn scroll_to_reveal_scroll_up() {
        let model = make_model("a\nb\nc\nd\ne");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let vp = Viewport::new(3, 2); // lines 3-4 visible
        let new_first = vm.scroll_to_reveal(1, &vp).unwrap();
        assert_eq!(new_first, 1);
    }

    #[test]
    fn scroll_to_reveal_scroll_down() {
        let model = make_model("a\nb\nc\nd\ne");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let vp = Viewport::new(1, 2); // lines 1-2 visible
        let new_first = vm.scroll_to_reveal(5, &vp).unwrap();
        assert_eq!(new_first, 4); // lines 4-5 visible
    }

    #[test]
    fn scroll_to_center_works() {
        let model = make_model("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let first = vm.scroll_to_center(5, 4);
        assert_eq!(first, 3); // center line 5 in viewport of 4 lines
    }

    // ---- coordinate mapping tests ----

    #[test]
    fn map_model_to_view_with_wrap() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let (vl, vc) = vm.map_model_to_view_coords(1, 7);
        assert_eq!(vl, 2);
        assert_eq!(vc, 1);
    }

    #[test]
    fn map_view_to_model_with_wrap() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let (ml, mc) = vm.map_view_to_model_coords(2, 1);
        assert_eq!(ml, 1);
        assert_eq!(mc, 7);
    }

    #[test]
    fn wrapped_line_count_check() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        assert_eq!(vm.wrapped_line_count(), 1);
    }

    #[test]
    fn view_line_range_for_model_range_works() {
        let model = make_model("hello world\nfoo\nbar");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let (first, last) = vm.view_line_range_for_model_range(1, 2).unwrap();
        assert_eq!(first, 1);
        assert_eq!(last, 3); // "hello " + "world" + "foo"
        assert!(vm.view_line_range_for_model_range(99, 100).is_none());
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn viewport_scroll_down() {
        let vp = Viewport::new(1, 20);
        let scrolled = vp.scroll_down(5);
        assert_eq!(scrolled.first_view_line, 6);
        assert_eq!(scrolled.visible_line_count, 20);
    }

    #[test]
    fn viewport_scroll_up() {
        let vp = Viewport::new(10, 20);
        let scrolled = vp.scroll_up(5);
        assert_eq!(scrolled.first_view_line, 5);
        // Cannot scroll below 1
        let scrolled2 = vp.scroll_up(100);
        assert_eq!(scrolled2.first_view_line, 1);
    }

    #[test]
    fn viewport_is_empty() {
        assert!(Viewport::new(1, 0).is_empty());
        assert!(!Viewport::new(1, 10).is_empty());
    }

    #[test]
    fn visible_range_merge() {
        let a = VisibleRange::new(5, 15);
        let b = VisibleRange::new(10, 25);
        let merged = a.merge(&b);
        assert_eq!(merged.first_line, 5);
        assert_eq!(merged.last_line, 25);
    }

    #[test]
    fn view_line_is_continuation() {
        let wrapped = ViewLine {
            content: "world".to_string(),
            model_line: 1,
            model_start_column: 7,
            is_wrapped: true,
        };
        assert!(wrapped.is_continuation());
        let normal = ViewLine {
            content: "hello".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert!(!normal.is_continuation());
    }

    #[test]
    fn line_height_tracker_average_height() {
        let mut tracker = LineHeightTracker::new(20);
        tracker.set_height(0, 30);
        tracker.set_height(1, 20);
        tracker.set_height(2, 10);
        let avg = tracker.average_height(3);
        assert!((avg - 20.0).abs() < f64::EPSILON);
        // Empty case
        assert!((tracker.average_height(0) - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_height_tracker_has_custom_heights() {
        let tracker = LineHeightTracker::new(20);
        assert!(!tracker.has_custom_heights());
        let mut tracker2 = LineHeightTracker::new(20);
        tracker2.set_height(0, 30);
        assert!(tracker2.has_custom_heights());
    }

    #[test]
    fn view_model_stats_summary() {
        let stats = ViewModelStats {
            model_line_count: 100,
            view_line_count: 120,
            wrapped_line_count: 20,
            longest_view_line: 80,
        };
        let s = stats.summary();
        assert!(s.contains("100 model lines"));
        assert!(s.contains("120 view lines"));
        assert!(s.contains("20 wrapped"));
    }

    // ---- ViewLineRange tests ----

    #[test]
    fn view_line_range_contains_and_len() {
        let r = ViewLineRange::new(3, 7);
        assert_eq!(r.len(), 5);
        assert!(!r.is_empty());
        assert!(r.contains(3));
        assert!(r.contains(7));
        assert!(!r.contains(2));
        assert!(!r.contains(8));
    }

    #[test]
    fn view_line_range_empty() {
        let r = ViewLineRange::new(5, 3);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(!r.contains(4));
    }

    #[test]
    fn view_line_range_intersect() {
        let a = ViewLineRange::new(1, 5);
        let b = ViewLineRange::new(3, 8);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 3);
        assert_eq!(inter.end, 5);

        let c = ViewLineRange::new(6, 10);
        assert!(a.intersect(&c).is_none());
    }

    #[test]
    fn view_line_range_union() {
        let a = ViewLineRange::new(2, 5);
        let b = ViewLineRange::new(4, 9);
        let u = a.union(&b);
        assert_eq!(u.start, 2);
        assert_eq!(u.end, 9);
    }

    #[test]
    fn view_line_range_display_and_from() {
        let r = ViewLineRange::new(1, 10);
        assert_eq!(format!("{r}"), "[1..10]");
        let r2: ViewLineRange = (3u32, 7u32).into();
        assert_eq!(r2.start, 3);
        assert_eq!(r2.end, 7);
    }

    // ---- ViewportState tests ----

    #[test]
    fn viewport_state_scroll_to() {
        let mut state = ViewportState::new(20);
        assert_eq!(state.scroll_position, 1);
        state.scroll_to(5, 100);
        assert_eq!(state.scroll_position, 5);
        // Clamp to valid max
        state.scroll_to(200, 100);
        assert_eq!(state.scroll_position, 81); // 100 - 20 + 1
    }

    #[test]
    fn viewport_state_ensure_visible() {
        let mut state = ViewportState::new(10);
        state.cursor_view_line = 15;
        state.ensure_visible(100);
        // cursor at 15, viewport=10 → scroll_position should be 6..15
        assert_eq!(state.scroll_position, 6);

        // Already visible
        state.cursor_view_line = 10;
        state.ensure_visible(100);
        assert_eq!(state.scroll_position, 6);
    }

    #[test]
    fn viewport_state_is_line_visible() {
        let mut state = ViewportState::new(5);
        state.scroll_to(3, 20);
        assert!(state.is_line_visible(3));
        assert!(state.is_line_visible(7));
        assert!(!state.is_line_visible(2));
        assert!(!state.is_line_visible(8));
    }

    #[test]
    fn viewport_state_visible_range() {
        let mut state = ViewportState::new(5);
        state.scroll_to(3, 10);
        let r = state.visible_range(10);
        assert_eq!(r.start, 3);
        assert_eq!(r.end, 7);
    }

    // ---- ViewLineIterator tests ----

    #[test]
    fn view_line_iterator_all() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let items: Vec<_> = ViewLineIterator::new(&vm).collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].0, 1);
        assert_eq!(items[2].1.content, "foo");
    }

    #[test]
    fn view_line_iterator_wrapped_only() {
        let model = make_model("hello world\nfoo");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let items: Vec<_> = ViewLineIterator::new(&vm).wrapped_only().collect();
        assert_eq!(items.len(), 1);
        assert!(items[0].1.is_wrapped);
        assert_eq!(items[0].0, 2);
    }

    #[test]
    fn view_line_iterator_model_range() {
        let model = make_model("aaa\nbbb\nccc\nddd");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let items: Vec<_> = ViewLineIterator::new(&vm).model_line_range(2, 3).collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].1.content, "bbb");
        assert_eq!(items[1].1.content, "ccc");
    }

    // ---- ViewLine content analysis tests ----

    #[test]
    fn view_line_leading_whitespace() {
        let vl = ViewLine {
            content: "   hello".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert_eq!(vl.leading_whitespace(), 3);

        let vl2 = ViewLine {
            content: "hello".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert_eq!(vl2.leading_whitespace(), 0);
    }

    #[test]
    fn view_line_trimmed_content() {
        let vl = ViewLine {
            content: "  hello  ".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert_eq!(vl.trimmed_content(), "hello");
    }

    #[test]
    fn view_line_is_whitespace_only() {
        let blank = ViewLine {
            content: "   \t  ".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert!(blank.is_whitespace_only());

        let not_blank = ViewLine {
            content: "  x  ".to_string(),
            model_line: 1,
            model_start_column: 1,
            is_wrapped: false,
        };
        assert!(!not_blank.is_whitespace_only());
    }

    #[test]
    fn view_line_model_range_check() {
        let vl = ViewLine {
            content: "hello".to_string(),
            model_line: 3,
            model_start_column: 5,
            is_wrapped: true,
        };
        let r = vl.model_range();
        assert_eq!(r.start.line, 3);
        assert_eq!(r.start.column, 5);
        assert_eq!(r.end.line, 3);
        assert_eq!(r.end.column, 10); // 5 + 5
    }

    // ---- ViewLineRange iteration & clamping tests ----

    #[test]
    fn view_line_range_iter() {
        let r = ViewLineRange::new(3, 6);
        let items: Vec<u32> = r.iter().collect();
        assert_eq!(items, vec![3, 4, 5, 6]);

        let empty = ViewLineRange::new(5, 3);
        let items2: Vec<u32> = empty.iter().collect();
        assert!(items2.is_empty());
    }

    #[test]
    fn view_line_range_clamp_and_expand() {
        let r = ViewLineRange::new(1, 100);
        let clamped = r.clamp(50);
        assert_eq!(clamped.start, 1);
        assert_eq!(clamped.end, 50);

        let r2 = ViewLineRange::new(5, 10);
        let expanded = r2.expand(3, 20);
        assert_eq!(expanded.start, 2);
        assert_eq!(expanded.end, 13);

        // Expanding at boundaries
        let r3 = ViewLineRange::new(1, 5);
        let expanded2 = r3.expand(5, 8);
        assert_eq!(expanded2.start, 1);
        assert_eq!(expanded2.end, 8);
    }

    // ---- ViewModel content query tests ----

    #[test]
    fn vm_find_lines_containing() {
        let model = make_model("hello world\nfoo bar\nhello again");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let found = vm.find_lines_containing("hello");
        assert_eq!(found, vec![1, 3]);
        assert!(vm.find_lines_containing("").is_empty());
        assert!(vm.find_lines_containing("zzz").is_empty());
    }

    #[test]
    fn vm_find_exact_trimmed() {
        let model = make_model("  foo  \nbar\n  foo  ");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let found = vm.find_exact_trimmed("foo");
        assert_eq!(found, vec![1, 3]);
    }

    #[test]
    fn vm_blank_lines() {
        let model = make_model("hello\n   \nworld\n\t\t");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let blanks = vm.blank_lines();
        assert_eq!(blanks, vec![2, 4]);
    }

    #[test]
    fn vm_indentation_map() {
        let model = make_model("hello\n  world\n    foo");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        let indent = vm.indentation_map();
        assert_eq!(indent, vec![0, 2, 4]);
    }

    #[test]
    fn vm_max_column_and_total_chars() {
        let model = make_model("short\na longer line\nhi");
        let vm = ViewModel::new(model, 0, WordWrap::Off);
        assert_eq!(vm.max_column(), 13); // "a longer line"
        assert_eq!(vm.total_character_count(), 5 + 13 + 2);
    }

    #[test]
    fn vm_view_line_model_range() {
        let model = make_model("hello world");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let r = vm.view_line_model_range(2);
        assert_eq!(r.start.line, 1);
        assert_eq!(r.start.column, 7);
        assert_eq!(r.end.line, 1);
        assert_eq!(r.end.column, 12); // "world" = 5 chars, 7+5=12
    }

    // ---- ViewportState cursor movement tests ----

    #[test]
    fn viewport_state_move_cursor_up_down() {
        let mut state = ViewportState::new(5);
        state.cursor_view_line = 10;
        state.scroll_to(8, 20);

        state.move_cursor_up(3, 20);
        assert_eq!(state.cursor_view_line, 7);

        state.move_cursor_down(10, 20);
        assert_eq!(state.cursor_view_line, 17);

        // Clamp to 1
        state.move_cursor_up(100, 20);
        assert_eq!(state.cursor_view_line, 1);
        assert_eq!(state.scroll_position, 1);
    }

    #[test]
    fn viewport_state_page_up_down() {
        let mut state = ViewportState::new(5);
        state.cursor_view_line = 1;

        state.page_down(20);
        assert_eq!(state.cursor_view_line, 6);

        state.page_up(20);
        assert_eq!(state.cursor_view_line, 1);
    }

    #[test]
    fn viewport_state_go_to_top_bottom() {
        let mut state = ViewportState::new(5);
        state.cursor_view_line = 10;
        state.scroll_to(8, 50);

        state.go_to_bottom(50);
        assert_eq!(state.cursor_view_line, 50);

        state.go_to_top(50);
        assert_eq!(state.cursor_view_line, 1);
        assert_eq!(state.scroll_position, 1);
    }

    #[test]
    fn viewport_state_scroll_fraction() {
        let state = ViewportState::new(10);
        // Document fits in viewport → 0.0
        assert!((state.scroll_fraction(10) - 0.0).abs() < f64::EPSILON);
        assert!((state.scroll_fraction(5) - 0.0).abs() < f64::EPSILON);

        let mut state2 = ViewportState::new(10);
        // total=20, max_scroll=10, scroll_position=1 → 0.0
        assert!((state2.scroll_fraction(20) - 0.0).abs() < f64::EPSILON);
        // scroll to end
        state2.scroll_to(11, 20);
        assert!((state2.scroll_fraction(20) - 1.0).abs() < f64::EPSILON);
    }

    // ---- Viewport helper tests ----

    #[test]
    fn viewport_clamp() {
        let vp = Viewport::new(8, 10);
        let clamped = vp.clamp(12);
        assert_eq!(clamped.first_view_line, 8);
        assert_eq!(clamped.visible_line_count, 5); // only 5 lines left

        let vp2 = Viewport::new(20, 10);
        let clamped2 = vp2.clamp(5);
        assert_eq!(clamped2.first_view_line, 5);
        assert_eq!(clamped2.visible_line_count, 1);
    }

    #[test]
    fn viewport_center_line_and_to_range() {
        let vp = Viewport::new(5, 10);
        assert_eq!(vp.center_line(), 10); // 5 + 10/2

        let range = vp.to_range();
        assert_eq!(range.start, 5);
        assert_eq!(range.end, 14); // last_view_line
    }

    // ---- LineHeightTracker extended tests ----

    #[test]
    fn line_height_tracker_reset_and_accessors() {
        let mut tracker = LineHeightTracker::new(18);
        assert_eq!(tracker.default_height(), 18);
        assert_eq!(tracker.explicit_count(), 0);

        tracker.set_height(0, 30);
        tracker.set_height(5, 40);
        assert_eq!(tracker.explicit_count(), 6); // 0..=5 allocated

        tracker.reset();
        assert_eq!(tracker.explicit_count(), 0);
        assert_eq!(tracker.get_height(0), 18); // back to default
    }

    #[test]
    fn line_height_tracker_tallest_line() {
        let mut tracker = LineHeightTracker::new(20);
        tracker.set_height(2, 50);
        tracker.set_height(4, 30);
        assert_eq!(tracker.tallest_line(5), Some(2));
        assert_eq!(tracker.tallest_line(0), None);

        // All default → last line wins (>=)
        let tracker2 = LineHeightTracker::new(20);
        assert_eq!(tracker2.tallest_line(3), Some(2));
    }

    // -- ViewModelColumnCache tests --

    #[test]
    fn column_cache_empty() {
        let cache = ViewModelColumnCache::new();
        assert!(!cache.is_valid());
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.get_offset(0), None);
    }

    #[test]
    fn column_cache_rebuild() {
        let mut cache = ViewModelColumnCache::new();
        let view_lines = vec![
            ViewLine { content: "hello".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "world".into(), model_line: 1, model_start_column: 6, is_wrapped: true },
            ViewLine { content: "foo".into(), model_line: 2, model_start_column: 1, is_wrapped: false },
        ];
        cache.rebuild(&view_lines);
        assert!(cache.is_valid());
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get_offset(0), Some(0));
        assert_eq!(cache.get_offset(1), Some(5));
        assert_eq!(cache.get_offset(2), Some(0));
        assert_eq!(cache.max_offset(), 5);
    }

    #[test]
    fn column_cache_invalidate() {
        let mut cache = ViewModelColumnCache::new();
        let view_lines = vec![
            ViewLine { content: "a".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
        ];
        cache.rebuild(&view_lines);
        assert!(cache.is_valid());
        cache.invalidate();
        assert!(!cache.is_valid());
        assert_eq!(cache.get_offset(0), None);
    }

    #[test]
    fn column_cache_total_offset() {
        let mut cache = ViewModelColumnCache::new();
        let view_lines = vec![
            ViewLine { content: "abc".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "de".into(), model_line: 1, model_start_column: 4, is_wrapped: true },
        ];
        cache.rebuild(&view_lines);
        assert_eq!(cache.total_offset(), 3); // 0 + 3
    }

    // -- ViewModelScrollDelta tests --

    #[test]
    fn scroll_delta_zero() {
        let d = ViewModelScrollDelta::zero();
        assert!(d.is_zero());
        assert_eq!(d.abs_lines(), 0);
    }

    #[test]
    fn scroll_delta_negate() {
        let d = ViewModelScrollDelta::new(5, 1.5);
        let neg = d.negate();
        assert_eq!(neg.delta_lines, -5);
        assert!((neg.delta_columns - (-1.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_delta_add() {
        let a = ViewModelScrollDelta::new(3, 0.5);
        let b = ViewModelScrollDelta::new(-1, 0.25);
        let sum = a.add(&b);
        assert_eq!(sum.delta_lines, 2);
        assert!((sum.delta_columns - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_delta_scale() {
        let d = ViewModelScrollDelta::new(2, 0.5);
        let s = d.scale(3);
        assert_eq!(s.delta_lines, 6);
        assert!((s.delta_columns - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_delta_display() {
        let d = ViewModelScrollDelta::new(10, 0.33);
        let s = format!("{d}");
        assert!(s.contains("10"));
        assert!(s.contains("0.33"));
    }

    // -- ViewModelLineRangeCalculator tests --

    #[test]
    fn range_calc_visible_range_basic() {
        let calc = ViewModelLineRangeCalculator::new(100);
        assert_eq!(calc.visible_range(0, 20), (0, 20));
        assert_eq!(calc.visible_range(90, 20), (90, 100));
        assert_eq!(calc.visible_count(0, 20), 20);
    }

    #[test]
    fn range_calc_at_boundaries() {
        let calc = ViewModelLineRangeCalculator::new(50);
        assert!(calc.is_at_top(0));
        assert!(!calc.is_at_top(1));
        assert!(calc.is_at_bottom(40, 10));
        assert!(!calc.is_at_bottom(30, 10));
    }

    #[test]
    fn range_calc_clamp_scroll() {
        let calc = ViewModelLineRangeCalculator::new(50);
        assert_eq!(calc.clamp_scroll(100, 20), 30);
        assert_eq!(calc.clamp_scroll(0, 20), 0);
        // When viewport is larger than total, clamp to 0
        assert_eq!(calc.clamp_scroll(10, 60), 0);
    }

    #[test]
    fn range_calc_lines_above_below() {
        let calc = ViewModelLineRangeCalculator::new(100);
        assert_eq!(calc.lines_above(25), 25);
        assert_eq!(calc.lines_below(25, 20), 55);
    }

    #[test]
    fn range_calc_scroll_by() {
        let calc = ViewModelLineRangeCalculator::new(100);
        assert_eq!(calc.scroll_by(10, 5, 20), 15);
        assert_eq!(calc.scroll_by(10, -5, 20), 5);
        assert_eq!(calc.scroll_by(10, -20, 20), 0);
        assert_eq!(calc.scroll_by(10, 200, 20), 80);
    }

    // -- ViewModelCoordinateTransform tests --

    #[test]
    fn coord_transform_no_wrap() {
        let view_lines = vec![
            ViewLine { content: "hello".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "world".into(), model_line: 2, model_start_column: 1, is_wrapped: false },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        let coord = tx.model_to_view(1, 3).unwrap();
        assert_eq!(coord.view_line, 0);
        assert_eq!(coord.view_column, 3);
        let (ml, mc) = tx.view_to_model(&coord).unwrap();
        assert_eq!(ml, 1);
        assert_eq!(mc, 3);
    }

    #[test]
    fn coord_transform_wrapped() {
        let view_lines = vec![
            ViewLine { content: "hell".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "o wo".into(), model_line: 1, model_start_column: 5, is_wrapped: true },
            ViewLine { content: "rld".into(), model_line: 1, model_start_column: 9, is_wrapped: true },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        // Column 6 in model → view line 1, view column 2
        let coord = tx.model_to_view(1, 6).unwrap();
        assert_eq!(coord.view_line, 1);
        assert_eq!(coord.view_column, 2);
    }

    #[test]
    fn coord_transform_first_last() {
        let view_lines = vec![
            ViewLine { content: "ab".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "cd".into(), model_line: 1, model_start_column: 3, is_wrapped: true },
            ViewLine { content: "ef".into(), model_line: 2, model_start_column: 1, is_wrapped: false },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        assert_eq!(tx.first_view_line_for_model(1), Some(0));
        assert_eq!(tx.last_view_line_for_model(1), Some(1));
        assert_eq!(tx.first_view_line_for_model(2), Some(2));
        assert_eq!(tx.view_line_count_for_model(1), 2);
        assert_eq!(tx.view_line_count_for_model(2), 1);
        assert_eq!(tx.total_view_lines(), 3);
    }

    #[test]
    fn coord_transform_wrapped_model_lines() {
        let view_lines = vec![
            ViewLine { content: "hello".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "ab".into(), model_line: 2, model_start_column: 1, is_wrapped: false },
            ViewLine { content: "cd".into(), model_line: 2, model_start_column: 3, is_wrapped: true },
            ViewLine { content: "xyz".into(), model_line: 3, model_start_column: 1, is_wrapped: false },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        let wrapped = tx.wrapped_model_lines();
        assert_eq!(wrapped, vec![2]);
    }

    #[test]
    fn coord_transform_model_to_view_not_found() {
        let view_lines = vec![
            ViewLine { content: "hello".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        assert!(tx.model_to_view(99, 1).is_none());
    }

    #[test]
    fn coord_transform_view_to_model_out_of_range() {
        let view_lines = vec![
            ViewLine { content: "hello".into(), model_line: 1, model_start_column: 1, is_wrapped: false },
        ];
        let tx = ViewModelCoordinateTransform::new(&view_lines);
        let coord = ViewModelCoordinate::new(5, 1);
        assert!(tx.view_to_model(&coord).is_none());
    }


    // -- viewmodel additional tests -------------------------------------------

    #[test]
    fn x_viewmodel_panel_state_new() {
        let p = XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XViewmodelLayoutRegion::Sidebar);
    }

    #[test]
    fn x_viewmodel_panel_area() {
        let p = XViewmodelPanelState::new(XViewmodelLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_viewmodel_panel_toggle() {
        let mut p = XViewmodelPanelState::new(XViewmodelLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_viewmodel_panel_resize() {
        let mut p = XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_viewmodel_panel_is_narrow() {
        let mut p = XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_viewmodel_total_visible_area_basic() {
        let panels = vec![
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "a"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_viewmodel_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_viewmodel_total_visible_area_hidden() {
        let mut panels = vec![
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "a"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_viewmodel_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_viewmodel_count_in_region_basic() {
        let panels = vec![
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "a"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "b"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_viewmodel_count_in_region(&panels, XViewmodelLayoutRegion::Sidebar), 2);
        assert_eq!(x_viewmodel_count_in_region(&panels, XViewmodelLayoutRegion::Editor), 1);
        assert_eq!(x_viewmodel_count_in_region(&panels, XViewmodelLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_viewmodel_widest_panel_basic() {
        let mut panels = vec![
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "narrow"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_viewmodel_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_viewmodel_collapse_region_basic() {
        let mut panels = vec![
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "a"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Sidebar, "b"),
            XViewmodelPanelState::new(XViewmodelLayoutRegion::Editor, "c"),
        ];
        x_viewmodel_collapse_region(&mut panels, XViewmodelLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_viewmodel_layout_constraint_clamp() {
        let lc = XViewmodelLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_viewmodel_layout_constraint_satisfied() {
        let lc = XViewmodelLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_viewmodel_widest_panel_empty() {
        let panels: Vec<XViewmodelPanelState> = vec![];
        assert!(x_viewmodel_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_viewmodel_layout_region_eq() {
        assert_eq!(XViewmodelLayoutRegion::Sidebar, XViewmodelLayoutRegion::Sidebar);
        assert_ne!(XViewmodelLayoutRegion::Sidebar, XViewmodelLayoutRegion::Panel);
    }


    // -- viewmodel extended domain tests ----------------------------------------

    #[test]
    fn y_viewmodel_enum_index() {
        assert_eq!(YViewmodelViewModelChangeKind::Insert.index(), 0);
        assert_eq!(YViewmodelViewModelChangeKind::Update.index(), 1);
        assert_eq!(YViewmodelViewModelChangeKind::Delete.index(), 2);
        assert_eq!(YViewmodelViewModelChangeKind::Reset.index(), 3);
    }

    #[test]
    fn y_viewmodel_enum_label() {
        assert_eq!(YViewmodelViewModelChangeKind::Insert.label(), "Insert");
        assert_eq!(YViewmodelViewModelChangeKind::Update.label(), "Update");
        assert_eq!(YViewmodelViewModelChangeKind::Delete.label(), "Delete");
        assert_eq!(YViewmodelViewModelChangeKind::Reset.label(), "Reset");
    }

    #[test]
    fn y_viewmodel_enum_all() {
        let all = YViewmodelViewModelChangeKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_viewmodel_enum_is_default() {
        assert!(YViewmodelViewModelChangeKind::Insert.is_default());
        assert!(!YViewmodelViewModelChangeKind::Reset.is_default());
    }

    #[test]
    fn y_viewmodel_enum_display() {
        assert_eq!(format!("{}", YViewmodelViewModelChangeKind::Insert), "Insert");
    }

    #[test]
    fn y_viewmodel_struct_new() {
        let s = YViewmodelViewModelChangeLog::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_viewmodel_struct_clear() {
        let mut s = YViewmodelViewModelChangeLog::new();
        s.changes.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_viewmodel_fingerprint_deterministic() {
        let h1 = y_viewmodel_fingerprint("hello");
        let h2 = y_viewmodel_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_viewmodel_fingerprint("a"), y_viewmodel_fingerprint("b"));
    }

    #[test]
    fn y_viewmodel_truncate_short() {
        assert_eq!(y_viewmodel_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_viewmodel_truncate_long() {
        let r = y_viewmodel_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_viewmodel_normalize_key_basic() {
        assert_eq!(y_viewmodel_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_viewmodel_split_path_basic() {
        let parts = y_viewmodel_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_viewmodel_count_occurrences_basic() {
        assert_eq!(y_viewmodel_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_viewmodel_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_viewmodel_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_viewmodel_in_range_basic() {
        assert!(y_viewmodel_in_range(5, 1, 10));
        assert!(y_viewmodel_in_range(1, 1, 10));
        assert!(y_viewmodel_in_range(10, 1, 10));
        assert!(!y_viewmodel_in_range(0, 1, 10));
        assert!(!y_viewmodel_in_range(11, 1, 10));
    }

    #[test]
    fn y_viewmodel_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_viewmodel_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_viewmodel_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_viewmodel_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- viewmodel Z-extended tests -----------------------------------------------

    #[test]
    fn z_viewmodel_priority_weight() {
        assert_eq!(ZViewmodelPriority::Idle.weight(), 0);
        assert_eq!(ZViewmodelPriority::Normal.weight(), 2);
        assert_eq!(ZViewmodelPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_viewmodel_priority_label() {
        assert_eq!(ZViewmodelPriority::Low.label(), "low");
        assert_eq!(ZViewmodelPriority::High.label(), "high");
    }

    #[test]
    fn z_viewmodel_priority_is_elevated() {
        assert!(!ZViewmodelPriority::Normal.is_elevated());
        assert!(ZViewmodelPriority::High.is_elevated());
        assert!(ZViewmodelPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_viewmodel_priority_display() {
        assert_eq!(format!("{}", ZViewmodelPriority::Idle), "idle");
    }

    #[test]
    fn z_viewmodel_priority_all_asc() {
        let all = ZViewmodelPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZViewmodelPriority::Idle);
        assert_eq!(all[4], ZViewmodelPriority::Realtime);
    }

    #[test]
    fn z_viewmodel_struct_new() {
        let s = ZViewmodelViewModelSnapshot::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_viewmodel_struct_toggled_clone() {
        let s = ZViewmodelViewModelSnapshot::new();
        let t = s.toggled_clone();
        assert_ne!(s.frozen, t.frozen);
    }

    #[test]
    fn z_viewmodel_rolling_hash_deterministic() {
        let h1 = z_viewmodel_rolling_hash(b"test");
        let h2 = z_viewmodel_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_viewmodel_rolling_hash(b"a"), z_viewmodel_rolling_hash(b"b"));
    }

    #[test]
    fn z_viewmodel_pad_to_basic() {
        assert_eq!(z_viewmodel_pad_to("hi", 5), "hi   ");
        assert_eq!(z_viewmodel_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_viewmodel_is_identifier_basic() {
        assert!(z_viewmodel_is_identifier("foo_bar"));
        assert!(z_viewmodel_is_identifier("abc123"));
        assert!(!z_viewmodel_is_identifier(""));
        assert!(!z_viewmodel_is_identifier("has space"));
    }

    #[test]
    fn z_viewmodel_levenshtein_basic() {
        assert_eq!(z_viewmodel_levenshtein("", ""), 0);
        assert_eq!(z_viewmodel_levenshtein("abc", "abc"), 0);
        assert_eq!(z_viewmodel_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_viewmodel_unique_words_basic() {
        let w = z_viewmodel_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_viewmodel_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_viewmodel_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_viewmodel_common_prefix_basic() {
        assert_eq!(z_viewmodel_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_viewmodel_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_viewmodel_struct_clear() {
        let mut s = ZViewmodelViewModelSnapshot::new();
        s.fields.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_viewmodel_rolling_hash_empty() {
        let h = z_viewmodel_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_87_push_and_len() {
        let mut rb = super::XbRingBuffer87::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_87_overwrite() {
        let mut rb = super::XbRingBuffer87::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_87_get_out_of_bounds() {
        let rb = super::XbRingBuffer87::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_87_drain_all() {
        let mut rb = super::XbRingBuffer87::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_87_peek_front_back() {
        let mut rb = super::XbRingBuffer87::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_87_clear() {
        let mut rb = super::XbRingBuffer87::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_87_capacity() {
        let rb = super::XbRingBuffer87::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_87_basic() {
        let h = super::xb_fnv1a_87(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_87(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_87_different_inputs() {
        let h1 = super::xb_fnv1a_87(b"abc");
        let h2 = super::xb_fnv1a_87(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_87_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_87(&data);
        let dec = super::xb_rle_decode_87(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_87_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_87(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_87(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_87_values() {
        assert!((super::xb_clamp_87(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_87(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_87(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_87_values() {
        assert!((super::xb_lerp_87(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_87(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_87(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_87_wrap_around_twice() {
        let mut rb = super::XbRingBuffer87::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }

}
