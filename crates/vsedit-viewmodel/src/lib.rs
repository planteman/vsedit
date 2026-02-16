use std::fmt;
use std::sync::Arc;

use vsedit_editor_config::WordWrap;
use vsedit_editor_types::{ITextModel, Position};
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
    fn scroll_to_center() {
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
    fn view_line_range_for_model_range() {
        let model = make_model("hello world\nfoo\nbar");
        let vm = ViewModel::new(model, 6, WordWrap::On);
        let (first, last) = vm.view_line_range_for_model_range(1, 2).unwrap();
        assert_eq!(first, 1);
        assert_eq!(last, 3); // "hello " + "world" + "foo"
        assert!(vm.view_line_range_for_model_range(99, 100).is_none());
    }
}
