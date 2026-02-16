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
use vsedit_find::{FindMatch, FindOptions, FindState};
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

    // Find/replace state
    pub show_find: bool,
    pub find_input: String,
    pub replace_input: String,
    pub show_replace: bool,
    pub find_state: FindState,
    pub find_is_regex: bool,
    pub find_is_case_sensitive: bool,
    pub find_is_whole_word: bool,
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
            show_find: false,
            find_input: String::new(),
            replace_input: String::new(),
            show_replace: false,
            find_state: FindState::new(),
            find_is_regex: false,
            find_is_case_sensitive: false,
            find_is_whole_word: false,
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

    // -- Find / Replace -------------------------------------------------------

    /// Show the find overlay.
    pub fn open_find(&mut self) {
        self.show_find = true;
    }

    /// Hide the find overlay and clear highlights.
    pub fn close_find(&mut self) {
        self.show_find = false;
        self.show_replace = false;
        self.find_state.matches.clear();
        self.find_state.current_match = None;
    }

    /// Toggle the replace input visibility.
    pub fn toggle_replace(&mut self) {
        self.show_replace = !self.show_replace;
    }

    /// Recompute matches from the current find_input and options.
    pub fn update_find_matches(&mut self) {
        let opts = FindOptions::new(&self.find_input)
            .with_regex(self.find_is_regex)
            .with_case_sensitive(self.find_is_case_sensitive)
            .with_whole_word(self.find_is_whole_word);
        self.find_state.options = opts;
        let text = self.model.get_value();
        self.find_state.search(&text);
    }

    /// Navigate to the next match and scroll it into view.
    pub fn find_next(&mut self) {
        self.find_state.next_match();
        self.scroll_to_current_match();
    }

    /// Navigate to the previous match and scroll it into view.
    pub fn find_previous(&mut self) {
        self.find_state.previous_match();
        self.scroll_to_current_match();
    }

    /// Replace the current match with `replace_input`.
    pub fn replace_current(&mut self) {
        if let Some(fm) = self.find_state.current().cloned() {
            let text = self.model.get_value();
            // Build new text with the current match replaced
            let mut new_text = String::with_capacity(text.len());
            for (line_idx, line) in text.lines().enumerate() {
                let line_num = (line_idx + 1) as u32;
                if line_num == fm.line {
                    let start = (fm.start_col - 1) as usize;
                    let end = (fm.end_col - 1) as usize;
                    new_text.push_str(&line[..start]);
                    new_text.push_str(&self.replace_input);
                    new_text.push_str(&line[end..]);
                } else {
                    new_text.push_str(line);
                }
                if line_idx + 1 < text.lines().count() {
                    new_text.push('\n');
                }
            }
            // Handle trailing newline
            if text.ends_with('\n') {
                new_text.push('\n');
            }
            self.model = Arc::new(TextModel::new(&new_text));
            self.view_model = ViewModel::new(self.model.clone(), 0, WordWrap::Off);
            self.update_find_matches();
        }
    }

    /// Replace all matches with `replace_input`.
    pub fn replace_all(&mut self) {
        let opts = FindOptions::new(&self.find_input)
            .with_regex(self.find_is_regex)
            .with_case_sensitive(self.find_is_case_sensitive)
            .with_whole_word(self.find_is_whole_word);
        let text = self.model.get_value();
        let new_text = vsedit_find::replace_all(&text, &opts, &self.replace_input);
        self.model = Arc::new(TextModel::new(&new_text));
        self.view_model = ViewModel::new(self.model.clone(), 0, WordWrap::Off);
        self.update_find_matches();
    }

    /// Access find matches.
    pub fn find_matches(&self) -> &[FindMatch] {
        &self.find_state.matches
    }

    /// Current match index.
    pub fn current_match_index(&self) -> Option<usize> {
        self.find_state.current_match
    }

    fn scroll_to_current_match(&mut self) {
        if let Some(fm) = self.find_state.current() {
            let pos = Position::new(fm.line, fm.start_col);
            let state = vsedit_cursor::CursorState::from_position(pos);
            self.cursor.set_state(0, state);
            self.ensure_cursor_visible();
        }
    }

    /// Height of the find bar in rows.
    fn find_bar_height(&self) -> u16 {
        if !self.show_find {
            0
        } else if self.show_replace {
            2
        } else {
            1
        }
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
        self.renderer.viewport.height = self.viewport_height as u32;
        self.renderer.line_number_width =
            EditorRenderer::line_number_width_for(self.model.get_line_count());
    }

    /// Render the editor into a ratatui buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        // Render find bar at the top if visible
        let find_bar_h = self.find_bar_height();
        if find_bar_h > 0 && area.height > find_bar_h {
            self.render_find_bar(
                Rect::new(area.x, area.y, area.width, find_bar_h),
                buf,
            );
        }

        // Adjust editor area below the find bar
        let editor_area = if find_bar_h > 0 && area.height > find_bar_h {
            Rect::new(area.x, area.y + find_bar_h, area.width, area.height - find_bar_h)
        } else {
            area
        };

        self.render_editor(editor_area, buf);
    }

    fn render_find_bar(&self, area: Rect, buf: &mut Buffer) {
        let bar_style = Style::default().bg(Color::Rgb(60, 60, 60)).fg(Color::White);
        // Fill background
        for row in 0..area.height {
            for col in 0..area.width {
                let x = area.x + col;
                let y = area.y + row;
                buf[(x, y)].set_char(' ').set_style(bar_style);
            }
        }

        // Row 0: Find input
        let match_info = if self.find_input.is_empty() {
            String::new()
        } else {
            let total = self.find_state.matches.len();
            match self.find_state.current_match {
                Some(idx) if total > 0 => format!(" {} of {}", idx + 1, total),
                _ => format!(" 0 of {}", total),
            }
        };

        let flags = format!(
            "{}{}{}",
            if self.find_is_regex { ".*" } else { "" },
            if self.find_is_case_sensitive { "Aa" } else { "" },
            if self.find_is_whole_word { "W" } else { "" },
        );

        let find_line = format!("Find: {}{} {}", self.find_input, match_info, flags);
        for (i, ch) in find_line.chars().enumerate() {
            let x = area.x + i as u16;
            if x < area.x + area.width {
                buf[(x, area.y)].set_char(ch).set_style(bar_style);
            }
        }

        // Row 1: Replace input (if visible)
        if self.show_replace && area.height > 1 {
            let replace_line = format!("Replace: {}", self.replace_input);
            for (i, ch) in replace_line.chars().enumerate() {
                let x = area.x + i as u16;
                if x < area.x + area.width {
                    buf[(x, area.y + 1)].set_char(ch).set_style(bar_style);
                }
            }
        }
    }

    fn render_editor(&self, area: Rect, buf: &mut Buffer) {
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

                    // Find match highlighting
                    if self.show_find && !self.find_state.matches.is_empty() {
                        let col_1based = model_col;
                        for (mi, fm) in self.find_state.matches.iter().enumerate() {
                            if fm.line == model_line
                                && col_1based >= fm.start_col
                                && col_1based < fm.end_col
                            {
                                if Some(mi) == self.find_state.current_match {
                                    // Current match: orange
                                    style = style.bg(Color::Rgb(220, 150, 30));
                                } else {
                                    // Other matches: yellow
                                    style = style.bg(Color::Rgb(180, 180, 30));
                                }
                                break;
                            }
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

/// Controls cursor blink timing with configurable rates.
pub struct EditorCursorBlink {
    blink_rate_ms: u64,
    is_visible: bool,
    is_enabled: bool,
    last_toggle_ms: u64,
    blink_count: u64,
}

impl EditorCursorBlink {
    /// Create a new blink controller with the given rate in milliseconds.
    pub fn new(blink_rate_ms: u64) -> Self {
        Self {
            blink_rate_ms,
            is_visible: true,
            is_enabled: true,
            last_toggle_ms: 0,
            blink_count: 0,
        }
    }

    /// Set the blink rate in milliseconds.
    pub fn set_rate(&mut self, ms: u64) {
        self.blink_rate_ms = ms;
    }

    /// Get the current blink rate in milliseconds.
    pub fn rate(&self) -> u64 {
        self.blink_rate_ms
    }

    /// Advance the blink timer by `elapsed_ms`. Returns `true` if visibility changed.
    pub fn tick(&mut self, elapsed_ms: u64) -> bool {
        if !self.is_enabled {
            return false;
        }
        self.last_toggle_ms += elapsed_ms;
        if self.last_toggle_ms >= self.blink_rate_ms {
            self.last_toggle_ms -= self.blink_rate_ms;
            self.is_visible = !self.is_visible;
            self.blink_count += 1;
            true
        } else {
            false
        }
    }

    /// Whether the cursor is currently visible.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Reset blink state so the cursor is visible and the timer restarts.
    pub fn reset(&mut self) {
        self.is_visible = true;
        self.last_toggle_ms = 0;
    }

    /// Enable blinking.
    pub fn enable(&mut self) {
        self.is_enabled = true;
    }

    /// Disable blinking and force the cursor visible.
    pub fn disable(&mut self) {
        self.is_enabled = false;
        self.is_visible = true;
    }

    /// Whether blinking is enabled.
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    /// Total number of blink toggles since creation.
    pub fn blink_count(&self) -> u64 {
        self.blink_count
    }

    /// Force the cursor visible without resetting the timer.
    pub fn force_visible(&mut self) {
        self.is_visible = true;
    }
}

impl Default for EditorCursorBlink {
    fn default() -> Self {
        Self::new(530)
    }
}

/// Synchronizes scroll positions between paired editor instances.
pub struct EditorScrollSync {
    source_scroll_top: u32,
    source_scroll_left: u32,
    target_scroll_top: u32,
    target_scroll_left: u32,
    vertical_enabled: bool,
    horizontal_enabled: bool,
    offset_lines: i32,
    ratio: f64,
}

impl EditorScrollSync {
    /// Create a new sync controller with both axes enabled, ratio 1.0, offset 0.
    pub fn new() -> Self {
        Self {
            source_scroll_top: 0,
            source_scroll_left: 0,
            target_scroll_top: 0,
            target_scroll_left: 0,
            vertical_enabled: true,
            horizontal_enabled: true,
            offset_lines: 0,
            ratio: 1.0,
        }
    }

    /// Enable or disable vertical scroll syncing.
    pub fn set_vertical_enabled(&mut self, v: bool) {
        self.vertical_enabled = v;
    }

    /// Enable or disable horizontal scroll syncing.
    pub fn set_horizontal_enabled(&mut self, v: bool) {
        self.horizontal_enabled = v;
    }

    /// Set a fixed line offset applied to the target vertical scroll.
    pub fn set_offset_lines(&mut self, offset: i32) {
        self.offset_lines = offset;
    }

    /// Set the scroll ratio (clamped to 0.1..=10.0).
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio.clamp(0.1, 10.0);
    }

    /// Update the target scroll positions based on the given source positions.
    pub fn sync_from_source(&mut self, source_top: u32, source_left: u32) {
        self.source_scroll_top = source_top;
        self.source_scroll_left = source_left;

        if self.vertical_enabled {
            let scaled = (source_top as f64 * self.ratio) as i64 + self.offset_lines as i64;
            self.target_scroll_top = scaled.max(0) as u32;
        }

        if self.horizontal_enabled {
            self.target_scroll_left = (source_left as f64 * self.ratio) as u32;
        }
    }

    /// The computed target vertical scroll position.
    pub fn target_scroll_top(&self) -> u32 {
        self.target_scroll_top
    }

    /// The computed target horizontal scroll position.
    pub fn target_scroll_left(&self) -> u32 {
        self.target_scroll_left
    }

    /// Returns `true` if source and target positions are equal.
    pub fn is_synced(&self) -> bool {
        self.source_scroll_top == self.target_scroll_top
            && self.source_scroll_left == self.target_scroll_left
    }

    /// Reset all positions to zero.
    pub fn reset(&mut self) {
        self.source_scroll_top = 0;
        self.source_scroll_left = 0;
        self.target_scroll_top = 0;
        self.target_scroll_left = 0;
    }
}

impl Default for EditorScrollSync {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the word under/around a given column in a line of text.
/// Words are sequences of alphanumeric chars or underscores.
/// Returns `(word, start_col_1based, end_col_1based)` or `None` if no word at position.
pub fn editor_word_at_position(line: &str, column_1based: u32) -> Option<(String, u32, u32)> {
    if line.is_empty() || column_1based == 0 {
        return None;
    }
    let col = column_1based as usize;
    let chars: Vec<char> = line.chars().collect();
    if col > chars.len() {
        return None;
    }
    // Index is 0-based; column_1based points to the character at col-1
    let idx = col - 1;
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    if !is_word_char(chars[idx]) {
        return None;
    }
    // Expand left
    let mut start = idx;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    // Expand right
    let mut end = idx;
    while end + 1 < chars.len() && is_word_char(chars[end + 1]) {
        end += 1;
    }
    let word: String = chars[start..=end].iter().collect();
    Some((word, (start + 1) as u32, (end + 1) as u32))
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

    // -- Find / Replace tests -----------------------------------------------

    #[test]
    fn open_close_find() {
        let mut w = EditorWidget::new();
        assert!(!w.show_find);
        w.open_find();
        assert!(w.show_find);
        w.close_find();
        assert!(!w.show_find);
    }

    #[test]
    fn find_matches_in_text() {
        let mut w = EditorWidget::new();
        w.open_text("hello world\nhello rust\ngoodbye");
        w.find_input = "hello".to_string();
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        assert_eq!(w.current_match_index(), Some(0));
    }

    #[test]
    fn find_next_navigates() {
        let mut w = EditorWidget::new();
        w.open_text("aaa bbb aaa ccc aaa");
        w.find_input = "aaa".to_string();
        w.update_find_matches();
        assert_eq!(w.current_match_index(), Some(0));
        w.find_next();
        assert_eq!(w.current_match_index(), Some(1));
        w.find_next();
        assert_eq!(w.current_match_index(), Some(2));
        w.find_next(); // wraps
        assert_eq!(w.current_match_index(), Some(0));
    }

    #[test]
    fn find_previous_navigates() {
        let mut w = EditorWidget::new();
        w.open_text("aaa bbb aaa");
        w.find_input = "aaa".to_string();
        w.update_find_matches();
        assert_eq!(w.current_match_index(), Some(0));
        w.find_previous(); // wraps to last
        assert_eq!(w.current_match_index(), Some(1));
        w.find_previous();
        assert_eq!(w.current_match_index(), Some(0));
    }

    #[test]
    fn replace_current_match() {
        let mut w = EditorWidget::new();
        w.open_text("hello world hello");
        w.find_input = "hello".to_string();
        w.replace_input = "hi".to_string();
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        w.replace_current();
        assert_eq!(w.model().get_line_content(1), "hi world hello");
        assert_eq!(w.find_matches().len(), 1);
    }

    #[test]
    fn replace_all_matches() {
        let mut w = EditorWidget::new();
        w.open_text("hello world\nhello rust");
        w.find_input = "hello".to_string();
        w.replace_input = "hi".to_string();
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        w.replace_all();
        assert_eq!(w.model().get_line_content(1), "hi world");
        assert_eq!(w.model().get_line_content(2), "hi rust");
        assert_eq!(w.find_matches().len(), 0);
    }

    #[test]
    fn find_regex() {
        let mut w = EditorWidget::new();
        w.open_text("abc 123 def 456");
        w.find_input = r"\d+".to_string();
        w.find_is_regex = true;
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        assert_eq!(w.find_matches()[0].text, "123");
        assert_eq!(w.find_matches()[1].text, "456");
    }

    #[test]
    fn find_case_sensitive() {
        let mut w = EditorWidget::new();
        w.open_text("Hello hello HELLO");
        w.find_input = "Hello".to_string();
        w.find_is_case_sensitive = true;
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 1);
        assert_eq!(w.find_matches()[0].start_col, 1);
    }

    #[test]
    fn find_case_insensitive() {
        let mut w = EditorWidget::new();
        w.open_text("Hello hello HELLO");
        w.find_input = "hello".to_string();
        w.find_is_case_sensitive = false;
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 3);
    }

    #[test]
    fn find_whole_word() {
        let mut w = EditorWidget::new();
        w.open_text("he hello the he");
        w.find_input = "he".to_string();
        w.find_is_whole_word = true;
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
    }

    #[test]
    fn empty_search_returns_no_matches() {
        let mut w = EditorWidget::new();
        w.open_text("hello world");
        w.find_input = String::new();
        w.update_find_matches();
        assert!(w.find_matches().is_empty());
        assert_eq!(w.current_match_index(), None);
    }

    #[test]
    fn match_highlight_positions() {
        let mut w = EditorWidget::new();
        w.open_text("abcabc");
        w.find_input = "abc".to_string();
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        let m0 = &w.find_matches()[0];
        assert_eq!(m0.line, 1);
        assert_eq!(m0.start_col, 1);
        assert_eq!(m0.end_col, 4);
        let m1 = &w.find_matches()[1];
        assert_eq!(m1.line, 1);
        assert_eq!(m1.start_col, 4);
        assert_eq!(m1.end_col, 7);
    }

    #[test]
    fn toggle_replace_visibility() {
        let mut w = EditorWidget::new();
        assert!(!w.show_replace);
        w.toggle_replace();
        assert!(w.show_replace);
        w.toggle_replace();
        assert!(!w.show_replace);
    }

    #[test]
    fn render_with_find_bar_does_not_panic() {
        let mut w = EditorWidget::new();
        w.open_text("hello world\nhello rust");
        w.open_find();
        w.find_input = "hello".to_string();
        w.update_find_matches();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn render_with_find_and_replace_bar() {
        let mut w = EditorWidget::new();
        w.open_text("hello world");
        w.open_find();
        w.show_replace = true;
        w.find_input = "hello".to_string();
        w.replace_input = "hi".to_string();
        w.update_find_matches();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
        // Find bar should be 2 rows, check "Find:" appears on row 0
        let row0: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(row0.contains("Find:"));
        // Check "Replace:" appears on row 1
        let row1: String = (0..area.width)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect();
        assert!(row1.contains("Replace:"));
    }

    #[test]
    fn close_find_clears_matches() {
        let mut w = EditorWidget::new();
        w.open_text("hello world hello");
        w.open_find();
        w.find_input = "hello".to_string();
        w.update_find_matches();
        assert_eq!(w.find_matches().len(), 2);
        w.close_find();
        assert!(w.find_matches().is_empty());
        assert_eq!(w.current_match_index(), None);
        assert!(!w.show_find);
        assert!(!w.show_replace);
    }

    // ---- EditorCursorBlink tests ----

    #[test]
    fn cursor_blink_default_rate() {
        let b = EditorCursorBlink::default();
        assert_eq!(b.rate(), 530);
        assert!(b.is_visible());
        assert!(b.is_enabled());
    }

    #[test]
    fn cursor_blink_custom_rate() {
        let b = EditorCursorBlink::new(250);
        assert_eq!(b.rate(), 250);
    }

    #[test]
    fn cursor_blink_tick_toggles_visibility() {
        let mut b = EditorCursorBlink::new(100);
        assert!(b.is_visible());
        let changed = b.tick(100);
        assert!(changed);
        assert!(!b.is_visible());
    }

    #[test]
    fn cursor_blink_tick_no_toggle_before_rate() {
        let mut b = EditorCursorBlink::new(100);
        let changed = b.tick(50);
        assert!(!changed);
        assert!(b.is_visible());
    }

    #[test]
    fn cursor_blink_multiple_ticks() {
        let mut b = EditorCursorBlink::new(100);
        b.tick(100); // visible -> hidden
        b.tick(100); // hidden -> visible
        assert!(b.is_visible());
        assert_eq!(b.blink_count(), 2);
    }

    #[test]
    fn cursor_blink_reset() {
        let mut b = EditorCursorBlink::new(100);
        b.tick(100);
        assert!(!b.is_visible());
        b.reset();
        assert!(b.is_visible());
    }

    #[test]
    fn cursor_blink_disable_forces_visible() {
        let mut b = EditorCursorBlink::new(100);
        b.tick(100);
        assert!(!b.is_visible());
        b.disable();
        assert!(b.is_visible());
        // Tick should not toggle while disabled
        let changed = b.tick(200);
        assert!(!changed);
        assert!(b.is_visible());
    }

    #[test]
    fn cursor_blink_enable_after_disable() {
        let mut b = EditorCursorBlink::new(100);
        b.disable();
        b.enable();
        assert!(b.is_enabled());
        let changed = b.tick(100);
        assert!(changed);
    }

    #[test]
    fn cursor_blink_set_rate() {
        let mut b = EditorCursorBlink::new(100);
        b.set_rate(200);
        assert_eq!(b.rate(), 200);
        let changed = b.tick(100);
        assert!(!changed); // Not enough time at new rate
    }

    #[test]
    fn cursor_blink_force_visible() {
        let mut b = EditorCursorBlink::new(100);
        b.tick(100); // hidden
        assert!(!b.is_visible());
        b.force_visible();
        assert!(b.is_visible());
    }

    // ---- EditorScrollSync tests ----

    #[test]
    fn scroll_sync_default_state() {
        let s = EditorScrollSync::new();
        assert_eq!(s.target_scroll_top(), 0);
        assert_eq!(s.target_scroll_left(), 0);
        assert!(s.is_synced());
    }

    #[test]
    fn scroll_sync_basic_sync() {
        let mut s = EditorScrollSync::new();
        s.sync_from_source(10, 5);
        assert_eq!(s.target_scroll_top(), 10);
        assert_eq!(s.target_scroll_left(), 5);
        assert!(s.is_synced());
    }

    #[test]
    fn scroll_sync_with_ratio() {
        let mut s = EditorScrollSync::new();
        s.set_ratio(2.0);
        s.sync_from_source(10, 4);
        assert_eq!(s.target_scroll_top(), 20);
        assert_eq!(s.target_scroll_left(), 8);
    }

    #[test]
    fn scroll_sync_with_offset() {
        let mut s = EditorScrollSync::new();
        s.set_offset_lines(5);
        s.sync_from_source(10, 0);
        assert_eq!(s.target_scroll_top(), 15);
    }

    #[test]
    fn scroll_sync_negative_offset_clamps_to_zero() {
        let mut s = EditorScrollSync::new();
        s.set_offset_lines(-20);
        s.sync_from_source(5, 0);
        assert_eq!(s.target_scroll_top(), 0);
    }

    #[test]
    fn scroll_sync_vertical_disabled() {
        let mut s = EditorScrollSync::new();
        s.set_vertical_enabled(false);
        s.sync_from_source(10, 5);
        assert_eq!(s.target_scroll_top(), 0); // unchanged
        assert_eq!(s.target_scroll_left(), 5);
    }

    #[test]
    fn scroll_sync_horizontal_disabled() {
        let mut s = EditorScrollSync::new();
        s.set_horizontal_enabled(false);
        s.sync_from_source(10, 5);
        assert_eq!(s.target_scroll_top(), 10);
        assert_eq!(s.target_scroll_left(), 0); // unchanged
    }

    #[test]
    fn scroll_sync_ratio_clamped() {
        let mut s = EditorScrollSync::new();
        s.set_ratio(0.01);
        s.sync_from_source(100, 0);
        // ratio clamped to 0.1
        assert_eq!(s.target_scroll_top(), 10);

        s.set_ratio(20.0);
        s.sync_from_source(1, 0);
        // ratio clamped to 10.0
        assert_eq!(s.target_scroll_top(), 10);
    }

    #[test]
    fn scroll_sync_reset() {
        let mut s = EditorScrollSync::new();
        s.sync_from_source(50, 30);
        s.reset();
        assert_eq!(s.target_scroll_top(), 0);
        assert_eq!(s.target_scroll_left(), 0);
        assert!(s.is_synced());
    }

    #[test]
    fn scroll_sync_is_synced_false_with_ratio() {
        let mut s = EditorScrollSync::new();
        s.set_ratio(2.0);
        s.sync_from_source(10, 0);
        assert!(!s.is_synced()); // source=10, target=20
    }

    // ---- editor_word_at_position tests ----

    #[test]
    fn word_at_position_middle_of_word() {
        let result = editor_word_at_position("hello world", 3);
        assert_eq!(result, Some(("hello".to_string(), 1, 5)));
    }

    #[test]
    fn word_at_position_start_of_word() {
        let result = editor_word_at_position("hello world", 1);
        assert_eq!(result, Some(("hello".to_string(), 1, 5)));
    }

    #[test]
    fn word_at_position_end_of_word() {
        let result = editor_word_at_position("hello world", 5);
        assert_eq!(result, Some(("hello".to_string(), 1, 5)));
    }

    #[test]
    fn word_at_position_second_word() {
        let result = editor_word_at_position("hello world", 8);
        assert_eq!(result, Some(("world".to_string(), 7, 11)));
    }

    #[test]
    fn word_at_position_on_space() {
        let result = editor_word_at_position("hello world", 6);
        assert_eq!(result, None);
    }

    #[test]
    fn word_at_position_empty_line() {
        let result = editor_word_at_position("", 1);
        assert_eq!(result, None);
    }

    #[test]
    fn word_at_position_beyond_line() {
        let result = editor_word_at_position("hi", 5);
        assert_eq!(result, None);
    }

    #[test]
    fn word_at_position_underscore_word() {
        let result = editor_word_at_position("my_var = 42", 3);
        assert_eq!(result, Some(("my_var".to_string(), 1, 6)));
    }

    #[test]
    fn word_at_position_on_punctuation() {
        let result = editor_word_at_position("a.b", 2);
        assert_eq!(result, None);
    }

    #[test]
    fn word_at_position_single_char_word() {
        let result = editor_word_at_position("a b c", 3);
        assert_eq!(result, Some(("b".to_string(), 3, 3)));
    }

    #[test]
    fn word_at_position_column_zero_returns_none() {
        let result = editor_word_at_position("hello", 0);
        assert_eq!(result, None);
    }

    #[test]
    fn word_at_position_numeric_word() {
        let result = editor_word_at_position("val = 12345;", 8);
        assert_eq!(result, Some(("12345".to_string(), 7, 11)));
    }
}
