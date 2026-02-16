//! Side-by-side and inline diff editor widgets for the TUI.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::diff_result::{compute_diff, DiffResult};

/// A line in the side-by-side diff view.
#[derive(Debug, Clone)]
pub struct DiffViewLine {
    pub left_line_no: Option<u32>,
    pub left_text: String,
    pub right_line_no: Option<u32>,
    pub right_text: String,
    pub kind: DiffViewLineKind,
}

/// The kind of a diff view line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewLineKind {
    Equal,
    Added,
    Deleted,
    Changed,
    /// Collapsed region placeholder.
    Collapsed(u32),
}

/// A line in the inline diff view.
#[derive(Debug, Clone)]
pub struct InlineDiffLine {
    pub line_no: Option<u32>,
    pub text: String,
    pub kind: InlineDiffLineKind,
}

/// Kind of inline diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDiffLineKind {
    Equal,
    Added,
    Deleted,
}

/// Widget for rendering a side-by-side diff view.
#[derive(Debug)]
pub struct DiffEditorWidget {
    pub original_lines: Vec<String>,
    pub modified_lines: Vec<String>,
    pub diff: DiffResult,
    pub scroll_offset: u32,
    pub collapse_unchanged: bool,
    pub context_lines: u32,
}

impl DiffEditorWidget {
    /// Create a new diff editor from two texts.
    pub fn new(original: &str, modified: &str) -> Self {
        let diff = compute_diff(original, modified);
        Self {
            original_lines: original.lines().map(String::from).collect(),
            modified_lines: modified.lines().map(String::from).collect(),
            diff,
            scroll_offset: 0,
            collapse_unchanged: true,
            context_lines: 3,
        }
    }

    /// Build the view lines for the side-by-side display.
    pub fn build_view_lines(&self) -> Vec<DiffViewLine> {
        let mut view_lines = Vec::new();
        let mut old_idx: usize = 0;
        let mut new_idx: usize = 0;

        // Collect changed line sets for collapsing logic
        let mut changed_old_lines = std::collections::HashSet::new();
        let mut changed_new_lines = std::collections::HashSet::new();
        for hunk in &self.diff.hunks {
            let os = (hunk.old_range.start as usize).saturating_sub(1);
            for i in os..os + hunk.old_range.count as usize {
                changed_old_lines.insert(i);
            }
            let ns = (hunk.new_range.start as usize).saturating_sub(1);
            for i in ns..ns + hunk.new_range.count as usize {
                changed_new_lines.insert(i);
            }
        }

        for hunk in &self.diff.hunks {
            let hunk_old_start = (hunk.old_range.start as usize).saturating_sub(1);
            let hunk_new_start = (hunk.new_range.start as usize).saturating_sub(1);

            // Equal lines before this hunk
            let equal_end_old = hunk_old_start;
            let equal_end_new = hunk_new_start;
            let equal_count_old = equal_end_old.saturating_sub(old_idx);
            let equal_count_new = equal_end_new.saturating_sub(new_idx);
            let equal_count = equal_count_old.max(equal_count_new);

            if self.collapse_unchanged && equal_count > (self.context_lines as usize * 2) {
                // Show context_lines at start
                for i in 0..self.context_lines as usize {
                    let oi = old_idx + i;
                    let ni = new_idx + i;
                    view_lines.push(DiffViewLine {
                        left_line_no: Some(oi as u32 + 1),
                        left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                        right_line_no: Some(ni as u32 + 1),
                        right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                        kind: DiffViewLineKind::Equal,
                    });
                }
                let hidden = equal_count as u32 - self.context_lines * 2;
                view_lines.push(DiffViewLine {
                    left_line_no: None,
                    left_text: String::new(),
                    right_line_no: None,
                    right_text: String::new(),
                    kind: DiffViewLineKind::Collapsed(hidden),
                });
                // Show context_lines at end
                for i in 0..self.context_lines as usize {
                    let oi = equal_end_old - self.context_lines as usize + i;
                    let ni = equal_end_new - self.context_lines as usize + i;
                    view_lines.push(DiffViewLine {
                        left_line_no: Some(oi as u32 + 1),
                        left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                        right_line_no: Some(ni as u32 + 1),
                        right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                        kind: DiffViewLineKind::Equal,
                    });
                }
            } else {
                for i in 0..equal_count {
                    let oi = old_idx + i;
                    let ni = new_idx + i;
                    view_lines.push(DiffViewLine {
                        left_line_no: Some(oi as u32 + 1),
                        left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                        right_line_no: Some(ni as u32 + 1),
                        right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                        kind: DiffViewLineKind::Equal,
                    });
                }
            }

            // Emit the hunk changes
            let del_count = hunk.old_range.count as usize;
            let ins_count = hunk.new_range.count as usize;
            let max_count = del_count.max(ins_count);

            for i in 0..max_count {
                let has_del = i < del_count;
                let has_ins = i < ins_count;
                let oi = hunk_old_start + i;
                let ni = hunk_new_start + i;

                let kind = match (has_del, has_ins) {
                    (true, true) => DiffViewLineKind::Changed,
                    (true, false) => DiffViewLineKind::Deleted,
                    (false, true) => DiffViewLineKind::Added,
                    (false, false) => DiffViewLineKind::Equal,
                };

                view_lines.push(DiffViewLine {
                    left_line_no: if has_del { Some(oi as u32 + 1) } else { None },
                    left_text: if has_del {
                        self.original_lines.get(oi).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    },
                    right_line_no: if has_ins { Some(ni as u32 + 1) } else { None },
                    right_text: if has_ins {
                        self.modified_lines.get(ni).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    },
                    kind,
                });
            }

            old_idx = hunk_old_start + del_count;
            new_idx = hunk_new_start + ins_count;
        }

        // Remaining equal lines after last hunk
        let remaining_old = self.original_lines.len().saturating_sub(old_idx);
        let remaining_new = self.modified_lines.len().saturating_sub(new_idx);
        let remaining = remaining_old.max(remaining_new);

        if self.collapse_unchanged && remaining > (self.context_lines as usize * 2) {
            for i in 0..self.context_lines as usize {
                let oi = old_idx + i;
                let ni = new_idx + i;
                view_lines.push(DiffViewLine {
                    left_line_no: Some(oi as u32 + 1),
                    left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                    right_line_no: Some(ni as u32 + 1),
                    right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                    kind: DiffViewLineKind::Equal,
                });
            }
            let hidden = remaining as u32 - self.context_lines * 2;
            view_lines.push(DiffViewLine {
                left_line_no: None,
                left_text: String::new(),
                right_line_no: None,
                right_text: String::new(),
                kind: DiffViewLineKind::Collapsed(hidden),
            });
            for i in 0..self.context_lines as usize {
                let oi = old_idx + remaining_old - self.context_lines as usize + i;
                let ni = new_idx + remaining_new - self.context_lines as usize + i;
                view_lines.push(DiffViewLine {
                    left_line_no: Some(oi as u32 + 1),
                    left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                    right_line_no: Some(ni as u32 + 1),
                    right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                    kind: DiffViewLineKind::Equal,
                });
            }
        } else {
            for i in 0..remaining {
                let oi = old_idx + i;
                let ni = new_idx + i;
                view_lines.push(DiffViewLine {
                    left_line_no: Some(oi as u32 + 1),
                    left_text: self.original_lines.get(oi).cloned().unwrap_or_default(),
                    right_line_no: Some(ni as u32 + 1),
                    right_text: self.modified_lines.get(ni).cloned().unwrap_or_default(),
                    kind: DiffViewLineKind::Equal,
                });
            }
        }

        view_lines
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Scroll up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
}

/// Build inline diff lines (single pane, alternating old/new for changes).
pub fn build_inline_diff_lines(original: &str, modified: &str) -> Vec<InlineDiffLine> {
    let diff = compute_diff(original, modified);
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();
    let mut result = Vec::new();
    let mut old_idx: usize = 0;
    let mut _new_idx: usize = 0;

    for hunk in &diff.hunks {
        let hunk_old_start = (hunk.old_range.start as usize).saturating_sub(1);
        let hunk_new_start = (hunk.new_range.start as usize).saturating_sub(1);

        // Equal lines before hunk
        while old_idx < hunk_old_start {
            result.push(InlineDiffLine {
                line_no: Some(old_idx as u32 + 1),
                text: orig_lines.get(old_idx).unwrap_or(&"").to_string(),
                kind: InlineDiffLineKind::Equal,
            });
            old_idx += 1;
            _new_idx += 1;
        }

        // Deleted lines first, then added
        for i in 0..hunk.old_range.count as usize {
            let oi = hunk_old_start + i;
            result.push(InlineDiffLine {
                line_no: Some(oi as u32 + 1),
                text: orig_lines.get(oi).unwrap_or(&"").to_string(),
                kind: InlineDiffLineKind::Deleted,
            });
        }
        for i in 0..hunk.new_range.count as usize {
            let ni = hunk_new_start + i;
            result.push(InlineDiffLine {
                line_no: Some(ni as u32 + 1),
                text: mod_lines.get(ni).unwrap_or(&"").to_string(),
                kind: InlineDiffLineKind::Added,
            });
        }

        old_idx = hunk_old_start + hunk.old_range.count as usize;
        _new_idx = hunk_new_start + hunk.new_range.count as usize;
    }

    // Remaining equal lines
    while old_idx < orig_lines.len() {
        result.push(InlineDiffLine {
            line_no: Some(old_idx as u32 + 1),
            text: orig_lines.get(old_idx).unwrap_or(&"").to_string(),
            kind: InlineDiffLineKind::Equal,
        });
        old_idx += 1;
    }

    result
}

/// Render a side-by-side diff editor into a ratatui buffer.
pub fn render_diff_editor(area: Rect, buf: &mut Buffer, _diff_result: &DiffResult, original: &str, modified: &str) {
    let widget = DiffEditorWidget::new(original, modified);
    let view_lines = widget.build_view_lines();

    let gutter_width = 5u16;
    let half_width = area.width.saturating_sub(1) / 2;
    let left_content_width = half_width.saturating_sub(gutter_width);
    let right_x = area.x + half_width + 1;
    let right_content_width = area.width.saturating_sub(half_width + 1).saturating_sub(gutter_width);

    let deleted_style = Style::default().bg(Color::Red).fg(Color::White);
    let added_style = Style::default().bg(Color::Green).fg(Color::Black);
    let collapsed_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
    let gutter_style = Style::default().fg(Color::DarkGray);

    for (i, vl) in view_lines.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }

        match vl.kind {
            DiffViewLineKind::Collapsed(n) => {
                let text = format!("⋯ {} lines hidden ⋯", n);
                let x = area.x + half_width.saturating_sub(text.len() as u16 / 2);
                buf.set_string(x, y, &text, collapsed_style);
            }
            _ => {
                let (left_style, right_style) = match vl.kind {
                    DiffViewLineKind::Deleted => (deleted_style, Style::default()),
                    DiffViewLineKind::Added => (Style::default(), added_style),
                    DiffViewLineKind::Changed => (deleted_style, added_style),
                    _ => (Style::default(), Style::default()),
                };

                // Left gutter
                if let Some(ln) = vl.left_line_no {
                    let gutter_text = format!("{:>4} ", ln);
                    buf.set_string(area.x, y, &gutter_text, gutter_style);
                }
                // Left content
                let left_text: String = vl.left_text.chars().take(left_content_width as usize).collect();
                buf.set_string(area.x + gutter_width, y, &left_text, left_style);

                // Separator
                buf.set_string(area.x + half_width, y, "│", gutter_style);

                // Right gutter
                if let Some(ln) = vl.right_line_no {
                    let gutter_text = format!("{:>4} ", ln);
                    buf.set_string(right_x, y, &gutter_text, gutter_style);
                }
                // Right content
                let right_text: String = vl.right_text.chars().take(right_content_width as usize).collect();
                buf.set_string(right_x + gutter_width, y, &right_text, right_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_editor_widget_no_changes() {
        let w = DiffEditorWidget::new("a\nb\nc\n", "a\nb\nc\n");
        let lines = w.build_view_lines();
        assert!(lines.iter().all(|l| l.kind == DiffViewLineKind::Equal));
    }

    #[test]
    fn diff_editor_widget_with_changes() {
        let w = DiffEditorWidget::new("a\nb\nc\n", "a\nX\nc\n");
        let lines = w.build_view_lines();
        let changed = lines.iter().filter(|l| l.kind == DiffViewLineKind::Changed).count();
        assert!(changed >= 1);
    }

    #[test]
    fn diff_editor_widget_deletion() {
        let w = DiffEditorWidget::new("a\nb\nc\n", "a\nc\n");
        let lines = w.build_view_lines();
        let deleted = lines.iter().filter(|l| l.kind == DiffViewLineKind::Deleted).count();
        assert!(deleted >= 1);
    }

    #[test]
    fn diff_editor_widget_addition() {
        let w = DiffEditorWidget::new("a\nc\n", "a\nb\nc\n");
        let lines = w.build_view_lines();
        let added = lines.iter().filter(|l| l.kind == DiffViewLineKind::Added).count();
        assert!(added >= 1);
    }

    #[test]
    fn diff_editor_scroll() {
        let mut w = DiffEditorWidget::new("a\n", "b\n");
        assert_eq!(w.scroll_offset, 0);
        w.scroll_down();
        assert_eq!(w.scroll_offset, 1);
        w.scroll_up();
        assert_eq!(w.scroll_offset, 0);
        w.scroll_up();
        assert_eq!(w.scroll_offset, 0);
    }

    #[test]
    fn diff_editor_collapse_unchanged() {
        // Make a large file with one change in the middle
        let mut orig = String::new();
        let mut modif = String::new();
        for i in 0..50 {
            orig.push_str(&format!("line{}\n", i));
            if i == 25 {
                modif.push_str("CHANGED\n");
            } else {
                modif.push_str(&format!("line{}\n", i));
            }
        }
        let w = DiffEditorWidget::new(&orig, &modif);
        let lines = w.build_view_lines();
        let collapsed = lines.iter().filter(|l| matches!(l.kind, DiffViewLineKind::Collapsed(_))).count();
        assert!(collapsed >= 1);
    }

    #[test]
    fn inline_diff_lines_basic() {
        let lines = build_inline_diff_lines("a\nb\nc\n", "a\nX\nc\n");
        let deleted = lines.iter().filter(|l| l.kind == InlineDiffLineKind::Deleted).count();
        let added = lines.iter().filter(|l| l.kind == InlineDiffLineKind::Added).count();
        assert!(deleted >= 1);
        assert!(added >= 1);
    }

    #[test]
    fn inline_diff_lines_no_changes() {
        let lines = build_inline_diff_lines("a\nb\n", "a\nb\n");
        assert!(lines.iter().all(|l| l.kind == InlineDiffLineKind::Equal));
    }

    #[test]
    fn inline_diff_lines_all_deleted() {
        let lines = build_inline_diff_lines("a\nb\n", "");
        let deleted = lines.iter().filter(|l| l.kind == InlineDiffLineKind::Deleted).count();
        assert!(deleted >= 1);
    }

    #[test]
    fn inline_diff_lines_all_added() {
        let lines = build_inline_diff_lines("", "a\nb\n");
        let added = lines.iter().filter(|l| l.kind == InlineDiffLineKind::Added).count();
        assert!(added >= 1);
    }

    #[test]
    fn render_diff_editor_does_not_panic() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let diff = compute_diff("a\nb\nc\n", "a\nX\nc\n");
        render_diff_editor(area, &mut buf, &diff, "a\nb\nc\n", "a\nX\nc\n");
        // Just verify it doesn't panic
    }

    #[test]
    fn render_diff_editor_empty_area() {
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        let diff = compute_diff("a\n", "b\n");
        render_diff_editor(area, &mut buf, &diff, "a\n", "b\n");
    }
}
