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
}
