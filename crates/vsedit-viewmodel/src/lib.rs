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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 197
// ---------------------------------------------------------------------------

/// Generic object pool `Xc197Pool<T>`.
pub struct Xc197Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc197Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc197PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc197Pool<T> {
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
    pub fn stats(&self) -> Xc197PoolStats {
        Xc197PoolStats {
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

impl<T> Default for Xc197Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc197Scheduler`.
pub struct Xc197Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc197Scheduler {
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

impl Default for Xc197Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_197 hash for the given byte slice.
pub fn xc_197_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_197 convention.
pub fn xc_197_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe100 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe100Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe100PipelineError {
    pub stage: Xe100Stage,
    pub message: String,
}

impl std::fmt::Display for Xe100PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe100Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe100Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError>>>,
    stage_names: Vec<Xe100Stage>,
}

impl Xe100Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe100Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe100Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe100Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe100Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
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

    pub fn compose(mut self, other: Xe100Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe100CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe100CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe100Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe100CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe100CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe100Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe100CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_100_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe100CacheEntry {
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

    fn xe_100_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe100CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_100_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
    Ok(data)
}

pub fn xe_100_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_100_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_100_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_100_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe100PipelineError> {
    Err(Xe100PipelineError {
        stage: Xe100Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_98: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg98Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg98Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg98Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_98: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg98Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg98Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg98Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg98Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 196).
pub struct Xh196SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh196SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 238 as u64,
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

/// A compact bit set supporting boolean operations (variant 196).
pub struct Xh196BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh196BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 196).
pub struct Xi196Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi196Deque<T> {
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
pub struct Xi196Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi196Interval {
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

/// A simple interval tree (variant 196).
pub struct Xi196IntervalTree {
    xi_intervals: Vec<Xi196Interval>,
}

impl Xi196IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi196Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi196Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi196Interval) -> Vec<&Xi196Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi196Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi196Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi196Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi196Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi196Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi196Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 196) ---

/// Disjoint set / union-find for crate 196.
pub struct Xj196UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj196UnionFind {
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

const XJ196_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 196.
pub struct Xj196BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj196BTreeNode<K, V>>>,
    len: usize,
}

struct Xj196BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj196BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj196BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ196_BTREE_ORDER - 1
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
        let mid = XJ196_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj196BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj196BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj196BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj196BTreeNode::xj_new_leaf();
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


// --- xk_196 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk196SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk196SegmentTree {
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
pub struct Xk196DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk196DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_196).
#[derive(Debug, Clone)]
pub struct Xl196Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl196Rope {
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

/// Suffix array for efficient string searching (xl_196).
#[derive(Debug, Clone)]
pub struct Xl196SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl196SuffixArray {
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


    // ---- xc_ pool / scheduler tests – block 197 ----

    #[test]
    fn xc_197_pool_new_empty() {
        let pool: super::Xc197Pool<i32> = super::Xc197Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_197_pool_release_acquire() {
        let mut pool = super::Xc197Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_197_pool_acquire_empty() {
        let mut pool: super::Xc197Pool<i32> = super::Xc197Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_197_pool_full() {
        let mut pool = super::Xc197Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_197_pool_drain() {
        let mut pool = super::Xc197Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_197_pool_stats() {
        let mut pool = super::Xc197Pool::new(8);
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
    fn xc_197_pool_clear() {
        let mut pool = super::Xc197Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_197_pool_shrink() {
        let mut pool = super::Xc197Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_197_pool_default() {
        let pool: super::Xc197Pool<String> = super::Xc197Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_197_pool_extend() {
        let mut pool = super::Xc197Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_197_pool_retain() {
        let mut pool = super::Xc197Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_197_scheduler_round_robin() {
        let mut sched = super::Xc197Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_197_scheduler_empty() {
        let mut sched = super::Xc197Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_197_scheduler_reset() {
        let mut sched = super::Xc197Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_197_scheduler_add_remove() {
        let mut sched = super::Xc197Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_197_scheduler_targets() {
        let sched = super::Xc197Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_197_hash_empty() {
        assert_eq!(super::xc_197_hash(b""), 5381);
    }

    #[test]
    fn xc_197_hash_data() {
        let h = super::xc_197_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_197_hash(b"hello"), h);
    }

    #[test]
    fn xc_197_reverse_str() {
        assert_eq!(super::xc_197_reverse("abc"), "cba");
        assert_eq!(super::xc_197_reverse(""), "");
    }


    #[test]
    fn xe_100_pipeline_empty() {
        let p = super::Xe100Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_100_pipeline_parse_stage() {
        let p = super::Xe100Pipeline::new()
            .add_parse(super::xe_100_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_100_pipeline_transform_double() {
        let p = super::Xe100Pipeline::new()
            .add_transform(super::xe_100_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_100_pipeline_validate_reverse() {
        let p = super::Xe100Pipeline::new()
            .add_validate(super::xe_100_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_100_pipeline_emit_filter() {
        let p = super::Xe100Pipeline::new()
            .add_emit(super::xe_100_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_100_pipeline_multi_stage() {
        let p = super::Xe100Pipeline::new()
            .add_parse(super::xe_100_pipeline_identity)
            .add_transform(super::xe_100_pipeline_double)
            .add_validate(super::xe_100_pipeline_reverse)
            .add_emit(super::xe_100_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_100_pipeline_error_propagation() {
        let p = super::Xe100Pipeline::new()
            .add_parse(super::xe_100_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe100Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_100_pipeline_compose() {
        let p1 = super::Xe100Pipeline::new()
            .add_parse(super::xe_100_pipeline_identity);
        let p2 = super::Xe100Pipeline::new()
            .add_transform(super::xe_100_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_100_pipeline_error_display() {
        let e = super::Xe100PipelineError {
            stage: super::Xe100Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_100_cache_put_get() {
        let mut c = super::Xe100Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_100_cache_miss() {
        let mut c: super::Xe100Cache<&str, i32> = super::Xe100Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_100_cache_ttl_expiry() {
        let mut c = super::Xe100Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_100_cache_evict() {
        let mut c = super::Xe100Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_100_cache_capacity() {
        let mut c = super::Xe100Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_100_cache_stats() {
        let mut c = super::Xe100Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_100_cache_clear() {
        let mut c = super::Xe100Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_98 graph tests ------------------------------------------------

    #[test]
    fn xg_98_graph_empty() {
        let g = super::Xg98Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_98_graph_add_node() {
        let mut g = super::Xg98Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_98_graph_add_edge() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_98_graph_neighbors() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_98_graph_has_path() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_98_graph_self_path() {
        let g = super::Xg98Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_98_graph_topo_sort() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_98_graph_cycle_detect_false() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_98_graph_cycle_detect_true() {
        let mut g = super::Xg98Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_98 heap tests -------------------------------------------------

    #[test]
    fn xg_98_heap_empty() {
        let h: super::Xg98Heap<i32> = super::Xg98Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_98_heap_push_pop() {
        let mut h = super::Xg98Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_98_heap_peek() {
        let mut h = super::Xg98Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_98_heap_drain_sorted() {
        let mut h = super::Xg98Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_98_heap_merge() {
        let mut a = super::Xg98Heap::new();
        let mut b = super::Xg98Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_98_heap_default() {
        let h: super::Xg98Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_98_graph_default() {
        let g: super::Xg98Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh196_skip_insert_contains() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh196_skip_remove() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh196_skip_len() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh196_skip_range_query() {
        let mut sl = super::Xh196SkipList::xh_new(4);
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
    fn xh196_skip_floor_ceiling() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh196_skip_rank() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh196_skip_empty() {
        let sl = super::Xh196SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh196_skip_duplicates() {
        let mut sl = super::Xh196SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh196_bitset_set_test() {
        let mut bs = super::Xh196BitSet::xh_new(256);
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
    fn xh196_bitset_clear_count() {
        let mut bs = super::Xh196BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh196_bitset_and_or_xor() {
        let mut a = super::Xh196BitSet::xh_new(128);
        let mut b = super::Xh196BitSet::xh_new(128);
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
    fn xh196_bitset_iter_ones() {
        let mut bs = super::Xh196BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh196_bitset_first_last() {
        let mut bs = super::Xh196BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh196_bitset_empty() {
        let bs = super::Xh196BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi196_deque_push_pop_back() {
        let mut dq = super::Xi196Deque::xi_new(4);
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
    fn xi196_deque_push_pop_front() {
        let mut dq = super::Xi196Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi196_deque_mixed_ops() {
        let mut dq = super::Xi196Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi196_deque_get_and_split() {
        let mut dq = super::Xi196Deque::xi_new(8);
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
    fn xi196_deque_rotate_left() {
        let mut dq = super::Xi196Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi196_deque_rotate_right() {
        let mut dq = super::Xi196Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi196_deque_grow() {
        let mut dq = super::Xi196Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi196_deque_empty() {
        let dq = super::Xi196Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi196_interval_tree_insert_query() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi196Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi196Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi196_interval_tree_overlap() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi196Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi196Interval::xi_new(12, 20));
        let q = super::Xi196Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi196_interval_tree_remove() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi196Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi196_interval_tree_gaps() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi196Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi196Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi196Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi196Interval::xi_new(8, 10));
    }

    #[test]
    fn xi196_interval_tree_merge() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi196Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi196Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi196Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi196Interval::xi_new(10, 15));
    }

    #[test]
    fn xi196_interval_tree_all() {
        let mut tree = super::Xi196IntervalTree::xi_new();
        tree.xi_insert(super::Xi196Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi196Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi196_interval_tree_empty() {
        let tree = super::Xi196IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi196_interval_tree_contains_point() {
        let iv = super::Xi196Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 196) ---

    #[test]
    fn xj_196_uf_make_and_find() {
        let mut uf = super::Xj196UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_196_uf_union_connected() {
        let mut uf = super::Xj196UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_196_uf_component_count() {
        let mut uf = super::Xj196UnionFind::xj_new();
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
    fn xj_196_uf_component_size() {
        let mut uf = super::Xj196UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_196_uf_largest_component() {
        let mut uf = super::Xj196UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_196_uf_many_elements() {
        let mut uf = super::Xj196UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_196_uf_separate_components() {
        let mut uf = super::Xj196UnionFind::xj_new();
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
    fn xj_196_uf_path_compression() {
        let mut uf = super::Xj196UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_196_bt_insert_get() {
        let mut bt = super::Xj196BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_196_bt_contains_len() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_196_bt_replace() {
        let mut bt = super::Xj196BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_196_bt_remove() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_196_bt_keys_values() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_196_bt_range() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_196_bt_min_max() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_196_bt_many_inserts() {
        let mut bt = super::Xj196BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_196 segment tree tests ---

    #[test]
    fn xk_196_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_196_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk196SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_196_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_196_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_196_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_196_st_single_element() {
        let data = vec![42];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_196_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk196SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_196_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk196SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_196 disjoint intervals tests ---

    #[test]
    fn xk_196_di_add_and_count() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_196_di_merge_overlap() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_196_di_contains() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_196_di_remove() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_196_di_covered_length() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_196_di_gaps() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_196_di_merge_adjacent() {
        let mut di = super::Xk196DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_196_di_empty() {
        let di = super::Xk196DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_196_rope_new_empty() {
        let rope = super::Xl196Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_196_rope_from_str() {
        let rope = super::Xl196Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_196_rope_insert_at() {
        let mut rope = super::Xl196Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_196_rope_delete_range() {
        let mut rope = super::Xl196Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_196_rope_char_at() {
        let rope = super::Xl196Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_196_rope_split_concat() {
        let rope = super::Xl196Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_196_rope_line_count() {
        let rope = super::Xl196Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_196_rope_line_at() {
        let rope = super::Xl196Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_196_sa_build_and_search() {
        let sa = super::Xl196SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_196_sa_count() {
        let sa = super::Xl196SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_196_sa_longest_repeated() {
        let sa = super::Xl196SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_196_sa_all_positions() {
        let sa = super::Xl196SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_196_sa_len() {
        let sa = super::Xl196SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_196_sa_empty() {
        let sa = super::Xl196SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_196_rope_slice() {
        let rope = super::Xl196Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_196_sa_search_start() {
        let sa = super::Xl196SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
