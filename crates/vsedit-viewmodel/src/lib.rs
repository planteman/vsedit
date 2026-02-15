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
}
