//! The complete editor widget combining all editor subsystems.
//!
//! [`EditorWidget`] integrates the text model, cursor controller, view model,
//! and renderer into a single component that can be rendered to a Ratatui
//! terminal buffer.

use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use vsedit_cursor::CursorController;
use vsedit_editor_config::WordWrap;
use vsedit_editor_render::EditorRenderer;
use vsedit_editor_types::{ITextModel, Position};
use vsedit_editor_viewparts::{format_line_number, LineNumberMode};
use vsedit_text_model::TextModel;
use vsedit_text_render::{display_width, truncate_to_width};
use vsedit_viewmodel::ViewModel;

/// The complete editor widget integrating all editor subsystems.
pub struct EditorWidget {
    model: Arc<TextModel>,
    cursor: CursorController,
    view_model: ViewModel,
    renderer: EditorRenderer,

    // Scroll state
    pub scroll_top: u32,
    pub scroll_left: u32,

    // Configuration
    pub show_line_numbers: bool,
    pub line_number_mode: LineNumberMode,
    pub show_minimap: bool,
    pub tab_size: u32,
    pub is_focused: bool,
    pub is_readonly: bool,

    // Viewport dimensions (set by handle_resize)
    viewport_width: u16,
    viewport_height: u16,
}

impl EditorWidget {
    /// Create a new editor widget with empty content.
    pub fn new() -> Self {
        let model = Arc::new(TextModel::empty());
        let view_model = ViewModel::new(model.clone(), 0, WordWrap::Off);
        Self {
            model,
            cursor: CursorController::new(),
            view_model,
            renderer: EditorRenderer::new(),
            scroll_top: 0,
            scroll_left: 0,
            show_line_numbers: true,
            line_number_mode: LineNumberMode::Absolute,
            show_minimap: false,
            tab_size: 4,
            is_focused: false,
            is_readonly: false,
            viewport_width: 80,
            viewport_height: 24,
        }
    }

    /// Load new content into the editor, resetting cursor and scroll.
    pub fn open_text(&mut self, content: &str) {
        self.model = Arc::new(TextModel::new(content));
        self.cursor = CursorController::new();
        self.view_model = ViewModel::new(self.model.clone(), 0, WordWrap::Off);
        self.scroll_top = 0;
        self.scroll_left = 0;
        self.update_renderer();
    }

    /// Scroll the viewport so the primary cursor is visible.
    pub fn ensure_cursor_visible(&mut self) {
        let pos = self.cursor.get_primary().position();
        let view_pos = self.view_model.model_position_to_view_position(pos);
        // view_pos is 1-based; scroll_top is 0-based view-line index
        let cursor_view_line = view_pos.line.saturating_sub(1);
        let height = self.viewport_height as u32;

        if cursor_view_line < self.scroll_top {
            self.scroll_top = cursor_view_line;
        } else if height > 0 && cursor_view_line >= self.scroll_top + height {
            self.scroll_top = cursor_view_line - height + 1;
        }

        // Horizontal scrolling for the cursor column
        let content_width = self.content_area_width() as u32;
        let line_content = self.model.get_line_content(pos.line);
        // Compute display column (0-based) up to the cursor
        let prefix = if (pos.column as usize) <= line_content.len() + 1 {
            &line_content[..(pos.column.saturating_sub(1)) as usize]
        } else {
            line_content
        };
        let cursor_display_col = display_width(prefix, self.tab_size) as u32;

        if cursor_display_col < self.scroll_left {
            self.scroll_left = cursor_display_col;
        } else if content_width > 0 && cursor_display_col >= self.scroll_left + content_width {
            self.scroll_left = cursor_display_col - content_width + 1;
        }
    }

    /// Update viewport dimensions (e.g. on terminal resize).
    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.viewport_width = width;
        self.viewport_height = height;
        self.update_renderer();
    }

    /// The 1-based line number of the primary cursor.
    pub fn cursor_line(&self) -> u32 {
        self.cursor.get_primary().position().line
    }

    /// The 1-based column of the primary cursor.
    pub fn cursor_column(&self) -> u32 {
        self.cursor.get_primary().position().column
    }

    /// Total number of lines in the text model.
    pub fn line_count(&self) -> u32 {
        self.model.get_line_count()
    }

    /// Access the underlying text model.
    pub fn model(&self) -> &Arc<TextModel> {
        &self.model
    }

    /// Access the cursor controller.
    pub fn cursor_controller(&self) -> &CursorController {
        &self.cursor
    }

    /// Mutable access to the cursor controller.
    pub fn cursor_controller_mut(&mut self) -> &mut CursorController {
        &mut self.cursor
    }

    /// Access the view model.
    pub fn view_model(&self) -> &ViewModel {
        &self.view_model
    }

    // -- Private helpers ----------------------------------------------------

    fn gutter_width(&self) -> u16 {
        if self.show_line_numbers {
            EditorRenderer::line_number_width_for(self.model.get_line_count())
        } else {
            0
        }
    }

    fn content_area_width(&self) -> u16 {
        self.viewport_width.saturating_sub(self.gutter_width())
    }

    fn update_renderer(&mut self) {
        self.renderer.viewport_height = self.viewport_height as u32;
        self.renderer.line_number_width =
            EditorRenderer::line_number_width_for(self.model.get_line_count());
    }

    /// Render the editor into a ratatui buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let gutter_w = self.gutter_width();
        let content_w = area.width.saturating_sub(gutter_w);
        let current_line = self.cursor.get_primary().position().line;
        let current_col = self.cursor.get_primary().position().column;

        // Build selection range for highlighting
        let selection = self.cursor.get_primary().selection;
        let sel_range = selection.as_range();
        let has_selection = !sel_range.is_empty();

        let view_line_count = self.view_model.get_view_line_count();

        for row in 0..area.height {
            let view_line_idx = self.scroll_top + row as u32;
            if view_line_idx >= view_line_count {
                // Render tilde for lines past end-of-file
                if gutter_w > 0 {
                    let tilde_x = area.x + gutter_w.saturating_sub(2);
                    if tilde_x < area.x + area.width {
                        buf[(tilde_x, area.y + row)]
                            .set_char('~')
                            .set_style(Style::default().fg(Color::DarkGray));
                    }
                }
                continue;
            }

            let vl = self.view_model.get_view_line(view_line_idx + 1);
            let model_line = vl.model_line;

            // -- Gutter (line numbers) --
            if self.show_line_numbers && gutter_w > 0 {
                let num_str = if vl.is_wrapped {
                    String::new()
                } else {
                    format_line_number(model_line, current_line, self.line_number_mode)
                };

                let gutter_style = if model_line == current_line && !vl.is_wrapped {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                // Right-align the number in the gutter (leave 1 col gap on right)
                let display_width_gutter = (gutter_w.saturating_sub(1)) as usize;
                let padded = format!("{:>width$} ", num_str, width = display_width_gutter);
                for (i, ch) in padded.chars().take(gutter_w as usize).enumerate() {
                    let x = area.x + i as u16;
                    if x < area.x + area.width {
                        buf[(x, area.y + row)].set_char(ch).set_style(gutter_style);
                    }
                }
            }

            // -- Content area --
            let content_x = area.x + gutter_w;
            let line_content = &vl.content;
            let rendered = truncate_to_width(line_content, (self.scroll_left as usize) + content_w as usize, self.tab_size);
            // Skip scroll_left display columns
            let visible = Self::skip_display_cols(&rendered, self.scroll_left as usize);

            let is_current_line = model_line == current_line;
            let current_line_style = if is_current_line && self.is_focused {
                Style::default().bg(Color::Rgb(40, 40, 40))
            } else {
                Style::default()
            };

            // Fill the content area background for current line
            if is_current_line && self.is_focused {
                for col in 0..content_w {
                    let x = content_x + col;
                    if x < area.x + area.width {
                        buf[(x, area.y + row)].set_style(current_line_style);
                    }
                }
            }

            // Render text characters
            let mut display_col: u16 = 0;
            for ch in visible.chars() {
                if display_col >= content_w {
                    break;
                }
                let x = content_x + display_col;
                if x < area.x + area.width {
                    let model_col = vl.model_start_column + self.scroll_left + display_col as u32;

                    let mut style = current_line_style;

                    // Selection highlighting
                    if has_selection {
                        let pos = Position::new(model_line, model_col);
                        if sel_range.contains_position(&pos) {
                            style = style.bg(Color::Rgb(38, 79, 120));
                        }
                    }

                    // Cursor rendering (inverse video)
                    if is_current_line
                        && model_col == current_col
                        && self.is_focused
                        && !vl.is_wrapped
                    {
                        style = style.add_modifier(Modifier::REVERSED);
                    }

                    buf[(x, area.y + row)].set_char(ch).set_style(style);
                }
                display_col += 1;
            }

            // Render cursor on empty line or at end-of-line
            if self.is_focused && is_current_line && !vl.is_wrapped {
                let cursor_display = Self::cursor_display_col(
                    line_content,
                    current_col,
                    vl.model_start_column,
                    self.tab_size,
                );
                if cursor_display >= self.scroll_left {
                    let cursor_x_offset = (cursor_display - self.scroll_left) as u16;
                    if cursor_x_offset < content_w {
                        let x = content_x + cursor_x_offset;
                        if x < area.x + area.width {
                            let cell = &mut buf[(x, area.y + row)];
                            if cell.symbol() == " " || cell.symbol().is_empty() {
                                cell.set_char(' ');
                            }
                            cell.set_style(
                                Style::default()
                                    .add_modifier(Modifier::REVERSED)
                                    .bg(Color::Rgb(40, 40, 40)),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Skip `n` display columns from a rendered string, returning the remainder.
    fn skip_display_cols(text: &str, n: usize) -> &str {
        let mut skipped = 0;
        for (i, _ch) in text.char_indices() {
            if skipped >= n {
                return &text[i..];
            }
            skipped += 1;
        }
        if skipped >= n {
            return "";
        }
        ""
    }

    /// Compute the 0-based display column for the cursor position.
    fn cursor_display_col(
        line_content: &str,
        cursor_col: u32,
        model_start_col: u32,
        tab_size: u32,
    ) -> u32 {
        let offset = (cursor_col.saturating_sub(model_start_col)) as usize;
        let prefix = if offset <= line_content.len() {
            &line_content[..offset]
        } else {
            line_content
        };
        display_width(prefix, tab_size) as u32
    }
}

impl Default for EditorWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Allow rendering the widget directly via ratatui's `Widget` trait.
impl Widget for &EditorWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        EditorWidget::render(self, area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let w = EditorWidget::new();
        assert!(!w.is_focused);
        assert!(w.show_line_numbers);
        assert!(!w.is_readonly);
        assert!(!w.show_minimap);
        assert_eq!(w.tab_size, 4);
        assert_eq!(w.scroll_top, 0);
        assert_eq!(w.scroll_left, 0);
    }

    #[test]
    fn default_trait() {
        let w = EditorWidget::default();
        assert_eq!(w.line_count(), 1);
    }

    #[test]
    fn open_text_sets_content() {
        let mut w = EditorWidget::new();
        w.open_text("hello\nworld\nfoo");
        assert_eq!(w.line_count(), 3);
        assert_eq!(w.model().get_line_content(1), "hello");
        assert_eq!(w.model().get_line_content(2), "world");
        assert_eq!(w.model().get_line_content(3), "foo");
    }

    #[test]
    fn open_text_resets_cursor() {
        let mut w = EditorWidget::new();
        w.open_text("hello\nworld");
        assert_eq!(w.cursor_line(), 1);
        assert_eq!(w.cursor_column(), 1);
    }

    #[test]
    fn open_text_resets_scroll() {
        let mut w = EditorWidget::new();
        w.scroll_top = 10;
        w.scroll_left = 5;
        w.open_text("new content");
        assert_eq!(w.scroll_top, 0);
        assert_eq!(w.scroll_left, 0);
    }

    #[test]
    fn handle_resize_updates_viewport() {
        let mut w = EditorWidget::new();
        w.handle_resize(120, 40);
        assert_eq!(w.viewport_width, 120);
        assert_eq!(w.viewport_height, 40);
    }

    #[test]
    fn cursor_line_and_column() {
        let mut w = EditorWidget::new();
        w.open_text("hello\nworld\nfoo");
        // Default cursor is at (1,1)
        assert_eq!(w.cursor_line(), 1);
        assert_eq!(w.cursor_column(), 1);
    }

    #[test]
    fn ensure_cursor_visible_scrolls_down() {
        let mut w = EditorWidget::new();
        w.handle_resize(80, 5);
        w.open_text("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
        // Move cursor to line 8
        let state = vsedit_cursor::CursorState::from_position(Position::new(8, 1));
        w.cursor_controller_mut().set_state(0, state);
        w.ensure_cursor_visible();
        // scroll_top should have moved so line 8 (0-based index 7) is visible
        assert!(w.scroll_top + (w.viewport_height as u32) > 7);
        assert!(w.scroll_top <= 7);
    }

    #[test]
    fn ensure_cursor_visible_scrolls_up() {
        let mut w = EditorWidget::new();
        w.handle_resize(80, 5);
        w.open_text("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
        w.scroll_top = 6;
        // Cursor at line 2 should cause scroll up
        let state = vsedit_cursor::CursorState::from_position(Position::new(2, 1));
        w.cursor_controller_mut().set_state(0, state);
        w.ensure_cursor_visible();
        assert!(w.scroll_top <= 1);
    }

    #[test]
    fn render_does_not_panic_empty() {
        let w = EditorWidget::new();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn render_does_not_panic_with_content() {
        let mut w = EditorWidget::new();
        w.is_focused = true;
        w.open_text("hello\nworld");
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn render_shows_line_numbers() {
        let mut w = EditorWidget::new();
        w.open_text("aaa\nbbb\nccc");
        w.show_line_numbers = true;
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
        // Line number "1" should appear in the gutter area
        let gutter_content: String = (0..w.gutter_width())
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(gutter_content.contains('1'));
    }

    #[test]
    fn render_hides_line_numbers() {
        let mut w = EditorWidget::new();
        w.open_text("aaa\nbbb");
        w.show_line_numbers = false;
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
        // With gutter_width 0, content starts at column 0
        assert_eq!(w.gutter_width(), 0);
    }

    #[test]
    fn gutter_width_scales_with_line_count() {
        let mut w = EditorWidget::new();
        // 3 lines → small gutter
        w.open_text("a\nb\nc");
        let small = w.gutter_width();
        // 1000 lines → bigger gutter
        let big_content: String = (0..1000).map(|i| format!("line {}\n", i)).collect();
        w.open_text(&big_content);
        let big = w.gutter_width();
        assert!(big > small);
    }

    #[test]
    fn line_number_mode_relative() {
        let mut w = EditorWidget::new();
        w.line_number_mode = LineNumberMode::Relative;
        w.open_text("a\nb\nc\nd\ne");
        // Set cursor to line 3
        let state = vsedit_cursor::CursorState::from_position(Position::new(3, 1));
        w.cursor_controller_mut().set_state(0, state);
        w.is_focused = true;
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
        // We just verify it doesn't panic with relative mode
        assert_eq!(w.cursor_line(), 3);
    }

    #[test]
    fn render_past_eof_shows_tildes() {
        let mut w = EditorWidget::new();
        w.open_text("only one line");
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
        // Row 1..4 should be past EOF; check for tilde on row 1
        let gw = w.gutter_width();
        if gw >= 2 {
            let tilde_x = gw - 2;
            let symbol = buf[(tilde_x, 1)].symbol().to_string();
            assert_eq!(symbol, "~");
        }
    }

    #[test]
    fn widget_trait_renders() {
        let mut w = EditorWidget::new();
        w.open_text("test");
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        // Use the Widget trait impl
        (&w).render(area, &mut buf);
    }

    #[test]
    fn scroll_horizontal_right() {
        let mut w = EditorWidget::new();
        w.handle_resize(10, 5);
        let long_line = "a".repeat(100);
        w.open_text(&long_line);
        // Move cursor far right
        let state = vsedit_cursor::CursorState::from_position(Position::new(1, 50));
        w.cursor_controller_mut().set_state(0, state);
        w.ensure_cursor_visible();
        assert!(w.scroll_left > 0, "should have scrolled right");
    }

    #[test]
    fn view_model_accessible() {
        let mut w = EditorWidget::new();
        w.open_text("hello\nworld");
        assert_eq!(w.view_model().get_view_line_count(), 2);
    }
}
