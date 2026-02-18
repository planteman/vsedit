//! The complete editor widget combining all editor subsystems.
//!
//! [`EditorWidget`] integrates the text model, cursor controller, view model,
//! and renderer into a single component that can be rendered to a Ratatui
//! terminal buffer.

use std::fmt;
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


// ---------------------------------------------------------------------------
// EditorWidgetConfig - builder-pattern configuration
// ---------------------------------------------------------------------------

/// Configuration for an editor widget instance.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWidgetConfig {
    pub show_line_numbers: bool,
    pub show_minimap: bool,
    pub tab_size: u8,
    pub word_wrap: bool,
    pub readonly: bool,
}

impl Default for EditorWidgetConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_minimap: true,
            tab_size: 4,
            word_wrap: false,
            readonly: false,
        }
    }
}

impl EditorWidgetConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to show line numbers.
    pub fn with_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Set whether to show the minimap.
    pub fn with_minimap(mut self, show: bool) -> Self {
        self.show_minimap = show;
        self
    }

    /// Set the tab size (clamped to 1..=8).
    pub fn with_tab_size(mut self, size: u8) -> Self {
        self.tab_size = size.clamp(1, 8);
        self
    }

    /// Set whether to enable word wrap.
    pub fn with_word_wrap(mut self, wrap: bool) -> Self {
        self.word_wrap = wrap;
        self
    }

    /// Set whether the editor is read-only.
    pub fn with_readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Returns true if this config uses default values.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl std::fmt::Display for EditorWidgetConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Config(lines={}, minimap={}, tab={}, wrap={}, ro={})",
            self.show_line_numbers, self.show_minimap, self.tab_size, self.word_wrap, self.readonly
        )
    }
}

// ---------------------------------------------------------------------------
// WidgetSelection - a single selection range in the editor
// ---------------------------------------------------------------------------

/// A selection range within the editor represented by anchor and active positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetSelection {
    pub anchor_line: u32,
    pub anchor_col: u32,
    pub active_line: u32,
    pub active_col: u32,
}

impl WidgetSelection {
    /// Create a new selection.
    pub fn new(anchor_line: u32, anchor_col: u32, active_line: u32, active_col: u32) -> Self {
        Self { anchor_line, anchor_col, active_line, active_col }
    }

    /// Create a cursor (empty selection) at the given position.
    pub fn cursor(line: u32, col: u32) -> Self {
        Self::new(line, col, line, col)
    }

    /// Returns true if the selection is empty (cursor only).
    pub fn is_empty(&self) -> bool {
        self.anchor_line == self.active_line && self.anchor_col == self.active_col
    }

    /// The number of lines spanned by this selection.
    pub fn line_span(&self) -> u32 {
        let min_line = self.anchor_line.min(self.active_line);
        let max_line = self.anchor_line.max(self.active_line);
        max_line - min_line + 1
    }

    /// Returns true if the given position is within the selection range.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        let (start_line, start_col, end_line, end_col) = self.ordered();
        if line < start_line || line > end_line {
            return false;
        }
        if line == start_line && col < start_col {
            return false;
        }
        if line == end_line && col > end_col {
            return false;
        }
        true
    }

    /// Returns true if this selection overlaps with another.
    pub fn overlaps(&self, other: &WidgetSelection) -> bool {
        let (s1l, s1c, e1l, e1c) = self.ordered();
        let (s2l, s2c, e2l, e2c) = other.ordered();
        if (e1l, e1c) < (s2l, s2c) || (e2l, e2c) < (s1l, s1c) {
            return false;
        }
        true
    }

    fn ordered(&self) -> (u32, u32, u32, u32) {
        if (self.anchor_line, self.anchor_col) <= (self.active_line, self.active_col) {
            (self.anchor_line, self.anchor_col, self.active_line, self.active_col)
        } else {
            (self.active_line, self.active_col, self.anchor_line, self.anchor_col)
        }
    }
}

impl std::fmt::Display for WidgetSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "Cursor({}:{})", self.anchor_line, self.anchor_col)
        } else {
            write!(
                f,
                "Sel({}:{}-{}:{})",
                self.anchor_line, self.anchor_col, self.active_line, self.active_col
            )
        }
    }
}

// ---------------------------------------------------------------------------
// WidgetSelectionSet - manages multiple selections
// ---------------------------------------------------------------------------

/// Manages a set of selections in the editor.
#[derive(Debug, Clone, Default)]
pub struct WidgetSelectionSet {
    selections: Vec<WidgetSelection>,
}

impl WidgetSelectionSet {
    /// Create an empty selection set.
    pub fn new() -> Self {
        Self { selections: Vec::new() }
    }

    /// Add a selection.
    pub fn add(&mut self, sel: WidgetSelection) {
        self.selections.push(sel);
    }

    /// Remove the selection at the given index.
    pub fn remove(&mut self, index: usize) -> Option<WidgetSelection> {
        if index < self.selections.len() {
            Some(self.selections.remove(index))
        } else {
            None
        }
    }

    /// Merge overlapping selections in place.
    pub fn merge_overlapping(&mut self) {
        if self.selections.len() < 2 {
            return;
        }
        self.selections.sort_by(|a, b| {
            let ao = a.ordered();
            let bo = b.ordered();
            (ao.0, ao.1).cmp(&(bo.0, bo.1))
        });
        let mut merged: Vec<WidgetSelection> = vec![self.selections[0].clone()];
        for sel in &self.selections[1..] {
            let last = merged.last().unwrap();
            if last.overlaps(sel) {
                let lo = last.ordered();
                let so = sel.ordered();
                let start_line = lo.0.min(so.0);
                let start_col = if lo.0 == so.0 { lo.1.min(so.1) } else if lo.0 < so.0 { lo.1 } else { so.1 };
                let end_line = lo.2.max(so.2);
                let end_col = if lo.2 == so.2 { lo.3.max(so.3) } else if lo.2 > so.2 { lo.3 } else { so.3 };
                *merged.last_mut().unwrap() = WidgetSelection::new(start_line, start_col, end_line, end_col);
            } else {
                merged.push(sel.clone());
            }
        }
        self.selections = merged;
    }

    /// Iterate over selections.
    pub fn iter(&self) -> std::slice::Iter<'_, WidgetSelection> {
        self.selections.iter()
    }

    /// Number of selections.
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Returns true if there are no selections.
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }
}

// ---------------------------------------------------------------------------
// EditorStats - tracks editor activity
// ---------------------------------------------------------------------------

/// Tracks editor usage statistics.
#[derive(Debug, Clone, Default)]
pub struct EditorStats {
    pub keystrokes: u64,
    pub edits: u64,
    pub selections_changed: u64,
}

impl EditorStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a keystroke.
    pub fn record_keystroke(&mut self) {
        self.keystrokes += 1;
    }

    /// Record an edit operation.
    pub fn record_edit(&mut self) {
        self.edits += 1;
    }

    /// Record a selection change.
    pub fn record_selection_change(&mut self) {
        self.selections_changed += 1;
    }

    /// Total number of recorded actions.
    pub fn total_actions(&self) -> u64 {
        self.keystrokes + self.edits + self.selections_changed
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        self.keystrokes = 0;
        self.edits = 0;
        self.selections_changed = 0;
    }
}

impl std::fmt::Display for EditorStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Stats(keys={}, edits={}, sel_changes={})",
            self.keystrokes, self.edits, self.selections_changed
        )
    }
}
// ---------------------------------------------------------------------------
// VisibleRange - tracks which lines are currently visible in the viewport
// ---------------------------------------------------------------------------

/// Represents the range of lines currently visible in the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRange {
    /// 0-based index of the first visible line.
    pub scroll_top: u32,
    /// Number of visible rows.
    pub viewport_height: u32,
    /// Total number of lines in the document.
    pub total_lines: u32,
}

impl VisibleRange {
    /// Create a new visible range.
    pub fn new(scroll_top: u32, viewport_height: u32, total_lines: u32) -> Self {
        Self {
            scroll_top,
            viewport_height,
            total_lines,
        }
    }

    /// The 1-based line number of the first visible line.
    pub fn first_visible_line(&self) -> u32 {
        self.scroll_top + 1
    }

    /// The 1-based line number of the last visible line (clamped to total_lines).
    pub fn last_visible_line(&self) -> u32 {
        (self.scroll_top + self.viewport_height).min(self.total_lines)
    }

    /// Whether the viewport is scrolled to the very top.
    pub fn is_at_top(&self) -> bool {
        self.scroll_top == 0
    }

    /// Whether the viewport is scrolled to the very bottom.
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_top + self.viewport_height >= self.total_lines
    }

    /// Returns true if the given 1-based line number is visible.
    pub fn contains_line(&self, line_1based: u32) -> bool {
        if line_1based == 0 {
            return false;
        }
        let line_0based = line_1based - 1;
        line_0based >= self.scroll_top && line_0based < self.scroll_top + self.viewport_height
    }

    /// The percentage of the document currently visible (0.0..=100.0).
    pub fn visible_percentage(&self) -> f64 {
        if self.total_lines == 0 {
            return 100.0;
        }
        let visible = self.viewport_height.min(self.total_lines);
        (visible as f64 / self.total_lines as f64) * 100.0
    }

    /// The scroll position as a fraction (0.0..=1.0) for scrollbar rendering.
    pub fn scroll_fraction(&self) -> f64 {
        if self.total_lines <= self.viewport_height {
            return 0.0;
        }
        self.scroll_top as f64 / (self.total_lines - self.viewport_height) as f64
    }

    /// How many lines can still be scrolled down.
    pub fn lines_below(&self) -> u32 {
        self.total_lines
            .saturating_sub(self.scroll_top + self.viewport_height)
    }

    /// How many lines are above the viewport.
    pub fn lines_above(&self) -> u32 {
        self.scroll_top
    }
}

// ---------------------------------------------------------------------------
// BracketMatcher - finds matching brackets in a line
// ---------------------------------------------------------------------------

/// Utility for matching bracket pairs within a single line of text.
pub struct BracketMatcher;

impl BracketMatcher {
    const OPEN_BRACKETS: &'static [char] = &['(', '[', '{'];
    const CLOSE_BRACKETS: &'static [char] = &[')', ']', '}'];

    /// Find the matching bracket for the character at `column_1based`.
    /// Returns the 1-based column of the matching bracket, or `None`.
    pub fn find_matching_bracket(line: &str, column_1based: u32) -> Option<u32> {
        if column_1based == 0 {
            return None;
        }
        let chars: Vec<char> = line.chars().collect();
        let idx = (column_1based - 1) as usize;
        if idx >= chars.len() {
            return None;
        }
        let ch = chars[idx];

        if let Some(bracket_idx) = Self::OPEN_BRACKETS.iter().position(|&b| b == ch) {
            // Search forward for matching close bracket
            let close = Self::CLOSE_BRACKETS[bracket_idx];
            let mut depth = 1i32;
            for i in (idx + 1)..chars.len() {
                if chars[i] == ch {
                    depth += 1;
                } else if chars[i] == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i + 1) as u32);
                    }
                }
            }
            None
        } else if let Some(bracket_idx) = Self::CLOSE_BRACKETS.iter().position(|&b| b == ch) {
            // Search backward for matching open bracket
            let open = Self::OPEN_BRACKETS[bracket_idx];
            let mut depth = 1i32;
            for i in (0..idx).rev() {
                if chars[i] == ch {
                    depth += 1;
                } else if chars[i] == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i + 1) as u32);
                    }
                }
            }
            None
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// GutterInfo - detailed gutter layout computation
// ---------------------------------------------------------------------------

/// Detailed information about gutter layout and widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GutterInfo {
    /// Width allocated for line numbers (0 if hidden).
    pub line_number_width: u16,
    /// Width allocated for fold markers (0 or 1).
    pub fold_marker_width: u16,
    /// Width allocated for breakpoint indicators (0 or 2).
    pub breakpoint_width: u16,
}

impl GutterInfo {
    /// Compute the gutter layout for the given configuration.
    pub fn compute(
        line_count: u32,
        show_line_numbers: bool,
        show_fold_markers: bool,
        show_breakpoints: bool,
    ) -> Self {
        let line_number_width = if show_line_numbers {
            EditorRenderer::line_number_width_for(line_count)
        } else {
            0
        };
        Self {
            line_number_width,
            fold_marker_width: if show_fold_markers { 1 } else { 0 },
            breakpoint_width: if show_breakpoints { 2 } else { 0 },
        }
    }

    /// Total width of the gutter in columns.
    pub fn total_width(&self) -> u16 {
        self.line_number_width + self.fold_marker_width + self.breakpoint_width
    }

    /// Width available for content given a total editor width.
    pub fn content_width(&self, total_width: u16) -> u16 {
        total_width.saturating_sub(self.total_width())
    }
}

// ---------------------------------------------------------------------------
// MinimapData - generates minimap density data from lines
// ---------------------------------------------------------------------------

/// A single entry in the minimap representing one source line.
#[derive(Debug, Clone)]
pub struct MinimapEntry {
    /// The 1-based model line number.
    pub line: u32,
    /// Character density (ratio of non-whitespace to total width), 0.0..=1.0.
    pub density: f64,
    /// Indent level in characters.
    pub indent: u32,
}

/// Data structure for rendering a minimap / code overview.
#[derive(Debug, Clone)]
pub struct MinimapData {
    pub entries: Vec<MinimapEntry>,
}

impl MinimapData {
    /// Build minimap data from a slice of line contents.
    /// `max_width` is the reference width for density calculation.
    pub fn from_lines(lines: &[String], max_width: u16) -> Self {
        let entries = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let trimmed = line.trim_start();
                let indent = (line.len() - trimmed.len()) as u32;
                let non_ws = line.chars().filter(|c| !c.is_whitespace()).count();
                let density = if max_width > 0 {
                    (non_ws as f64 / max_width as f64).min(1.0)
                } else {
                    0.0
                };
                MinimapEntry {
                    line: (i + 1) as u32,
                    density,
                    indent,
                }
            })
            .collect();
        Self { entries }
    }

    /// Return the subset of entries visible in the given viewport range.
    pub fn visible_entries(&self, scroll_top: u32, height: u32) -> &[MinimapEntry] {
        let start = scroll_top as usize;
        let end = (scroll_top + height) as usize;
        let clamped_end = end.min(self.entries.len());
        if start >= self.entries.len() {
            return &[];
        }
        &self.entries[start..clamped_end]
    }
}

// ---------------------------------------------------------------------------
// IndentInfo - analyzes indentation of a line
// ---------------------------------------------------------------------------

/// Information about the indentation of a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndentInfo {
    /// Number of leading whitespace characters.
    pub level: u32,
    /// Whether tabs are used in the indentation.
    pub uses_tabs: bool,
    /// Number of leading tab characters.
    pub tab_count: u32,
    /// Number of leading space characters (after tabs).
    pub space_count: u32,
}

impl IndentInfo {
    /// Analyze the indentation of a line.
    pub fn from_line(line: &str) -> Self {
        let mut tabs = 0u32;
        let mut spaces = 0u32;
        let mut in_tabs = true;
        for ch in line.chars() {
            match ch {
                '\t' if in_tabs => tabs += 1,
                ' ' => {
                    in_tabs = false;
                    spaces += 1;
                }
                '\t' if !in_tabs => {
                    // Tab after spaces - count it but mark mixed
                    tabs += 1;
                }
                _ => break,
            }
        }
        Self {
            level: tabs + spaces,
            uses_tabs: tabs > 0,
            tab_count: tabs,
            space_count: spaces,
        }
    }

    /// Compute the visual width of this indentation given a tab size.
    pub fn visual_width(&self, tab_size: u32) -> u32 {
        self.tab_count * tab_size + self.space_count
    }

    /// Convert this indentation to a string using the specified style.
    pub fn to_string_with_style(&self, use_tabs: bool, tab_size: u32) -> String {
        if use_tabs {
            let full_tabs = self.visual_width(tab_size) / tab_size;
            let remaining = self.visual_width(tab_size) % tab_size;
            let mut s = "\t".repeat(full_tabs as usize);
            s.push_str(&" ".repeat(remaining as usize));
            s
        } else {
            " ".repeat(self.visual_width(tab_size) as usize)
        }
    }
}

// ---------------------------------------------------------------------------
// ViewportLineMap - maps between viewport rows and model lines
// ---------------------------------------------------------------------------

/// Maps between viewport row indices and 1-based model line numbers.
#[derive(Debug, Clone)]
pub struct ViewportLineMap {
    scroll_top: u32,
    viewport_height: u32,
    total_lines: u32,
}

impl ViewportLineMap {
    /// Create a new mapping.
    pub fn new(scroll_top: u32, viewport_height: u32, total_lines: u32) -> Self {
        Self {
            scroll_top,
            viewport_height,
            total_lines,
        }
    }

    /// Convert a viewport row (0-based) to a 1-based model line number.
    /// Returns `None` if the row is past the end of the document or viewport.
    pub fn viewport_to_model(&self, row: u32) -> Option<u32> {
        if row >= self.viewport_height {
            return None;
        }
        let line_0based = self.scroll_top + row;
        if line_0based >= self.total_lines {
            return None;
        }
        Some(line_0based + 1)
    }

    /// Convert a 1-based model line to a viewport row (0-based).
    /// Returns `None` if the line is not visible.
    pub fn model_to_viewport(&self, line_1based: u32) -> Option<u32> {
        if line_1based == 0 {
            return None;
        }
        let line_0based = line_1based - 1;
        if line_0based < self.scroll_top {
            return None;
        }
        let row = line_0based - self.scroll_top;
        if row >= self.viewport_height {
            return None;
        }
        Some(row)
    }

    /// The number of document lines that actually map to viewport rows.
    pub fn visible_line_count(&self) -> u32 {
        let available = self.total_lines.saturating_sub(self.scroll_top);
        available.min(self.viewport_height)
    }
}

// ---------------------------------------------------------------------------
// WidgetResizeHandler - widget resize handler
// ---------------------------------------------------------------------------

/// Severity level for widget resize handler issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WidgetResizeHandlerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for WidgetResizeHandlerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [WidgetResizeHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetResizeHandlerEntry {
    pub id: String,
    pub label: String,
    pub severity: WidgetResizeHandlerSeverity,
    pub detail: Option<String>,
    pub min_width: usize,
    enabled: bool,
}

impl WidgetResizeHandlerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: WidgetResizeHandlerSeverity::Low,
            detail: None,
            min_width: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: WidgetResizeHandlerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_min_width(mut self, val: usize) -> Self {
        self.min_width = val;
        self
    }

    pub fn is_resizable(&self) -> bool {
        self.enabled && self.severity >= WidgetResizeHandlerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.min_width, det)
    }
}

impl fmt::Display for WidgetResizeHandlerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [WidgetResizeHandlerEntry] items.
#[derive(Debug, Clone)]
pub struct WidgetResizeHandler {
    entries: Vec<WidgetResizeHandlerEntry>,
    name: String,
    capacity: usize,
}

impl WidgetResizeHandler {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: WidgetResizeHandlerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<WidgetResizeHandlerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&WidgetResizeHandlerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn min_width(&self) -> usize { self.entries.len() }

    pub fn is_resizable(&self) -> bool {
        self.entries.iter().any(|e| e.is_resizable())
    }

    pub fn entries_by_severity(&self, severity: WidgetResizeHandlerSeverity) -> Vec<&WidgetResizeHandlerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= WidgetResizeHandlerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&WidgetResizeHandlerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&WidgetResizeHandlerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// WidgetFocusCycleManager - widget focus cycle manager
// ---------------------------------------------------------------------------

/// Configuration for [WidgetFocusCycleManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetFocusCycleManagerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub widget_count: usize,
}

impl WidgetFocusCycleManagerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, widget_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_widget_count(mut self, val: usize) -> Self { self.widget_count = val; self }
}

impl Default for WidgetFocusCycleManagerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [WidgetFocusCycleManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetFocusCycleManagerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl WidgetFocusCycleManagerItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_focus(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for WidgetFocusCycleManagerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [WidgetFocusCycleManagerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct WidgetFocusCycleManager {
    config: WidgetFocusCycleManagerConfig,
    items: Vec<WidgetFocusCycleManagerItem>,
}

impl WidgetFocusCycleManager {
    pub fn new(config: WidgetFocusCycleManagerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: WidgetFocusCycleManagerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<WidgetFocusCycleManagerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&WidgetFocusCycleManagerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn widget_count(&self) -> usize { self.items.len() }

    pub fn has_focus(&self) -> bool {
        self.items.iter().any(|i| i.has_focus())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&WidgetFocusCycleManagerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&WidgetFocusCycleManagerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &WidgetFocusCycleManagerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}


/// Widget overlay management for editor chrome.
#[derive(Debug, Clone)]
pub struct WidgetOverlayRegistry {
    entries: Vec<WidgetOverlayItem>,
    enabled: bool,
    max_entries: usize,
}

/// A single widget overlay item.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetOverlayItem {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WidgetOverlayItem {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WidgetOverlayRegistry {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WidgetOverlayItem) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WidgetOverlayItem> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WidgetOverlayItem> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WidgetOverlayItem> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WidgetOverlayItem> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WidgetOverlayItem> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WidgetOverlayItem> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Editor widget composition — extended utilities (xa)
// ---------------------------------------------------------------------------

/// Metric accumulator for editor_wgt operations.
#[derive(Debug, Clone)]
pub struct XaMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XaMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for editor_wgt.
#[derive(Debug, Clone)]
pub struct XaRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XaRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for editor_wgt lookups.
#[derive(Debug, Clone)]
pub struct XaLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XaLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 17
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer17 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer17 {
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
pub fn xb_fnv1a_17(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_17<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_17<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_17(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_17(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 41
// ---------------------------------------------------------------------------

/// Generic object pool `Xc41Pool<T>`.
pub struct Xc41Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc41Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc41PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc41Pool<T> {
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
    pub fn stats(&self) -> Xc41PoolStats {
        Xc41PoolStats {
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

impl<T> Default for Xc41Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc41Scheduler`.
pub struct Xc41Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc41Scheduler {
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

impl Default for Xc41Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_41 hash for the given byte slice.
pub fn xc_41_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_41 convention.
pub fn xc_41_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe29 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe29Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe29PipelineError {
    pub stage: Xe29Stage,
    pub message: String,
}

impl std::fmt::Display for Xe29PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe29Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe29Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError>>>,
    stage_names: Vec<Xe29Stage>,
}

impl Xe29Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe29Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe29Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe29Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe29Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
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

    pub fn compose(mut self, other: Xe29Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe29CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe29CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe29Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe29CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe29CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe29Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe29CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_29_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe29CacheEntry {
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

    fn xe_29_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe29CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_29_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
    Ok(data)
}

pub fn xe_29_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_29_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_29_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_29_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe29PipelineError> {
    Err(Xe29PipelineError {
        stage: Xe29Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #115
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf115Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf115TrieNode {
    children: std::collections::HashMap<char, Xf115TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf115Trie {
    root: Xf115TrieNode,
    count: usize,
}

impl Xf115Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf115TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf115TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf115TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf115BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf115BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 40).
pub struct Xh40SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh40SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 82 as u64,
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

/// A compact bit set supporting boolean operations (variant 40).
pub struct Xh40BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh40BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 40).
pub struct Xi40Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi40Deque<T> {
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
pub struct Xi40Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi40Interval {
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

/// A simple interval tree (variant 40).
pub struct Xi40IntervalTree {
    xi_intervals: Vec<Xi40Interval>,
}

impl Xi40IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi40Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi40Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi40Interval) -> Vec<&Xi40Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi40Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi40Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi40Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi40Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi40Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi40Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 40) ---

/// Disjoint set / union-find for crate 40.
pub struct Xj40UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj40UnionFind {
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

const XJ40_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 40.
pub struct Xj40BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj40BTreeNode<K, V>>>,
    len: usize,
}

struct Xj40BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj40BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj40BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ40_BTREE_ORDER - 1
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
        let mid = XJ40_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj40BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj40BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj40BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj40BTreeNode::xj_new_leaf();
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


// --- xk_40 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk40SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk40SegmentTree {
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
pub struct Xk40DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk40DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_40).
#[derive(Debug, Clone)]
pub struct Xl40Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl40Rope {
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

/// Suffix array for efficient string searching (xl_40).
#[derive(Debug, Clone)]
pub struct Xl40SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl40SuffixArray {
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
pub struct Xm40MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm40MatrixSparse {
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
pub struct Xm40Tokenizer {
    text: String,
}

impl Xm40Tokenizer {
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

    #[test]
    fn editor_widget_config_builder() {
        let cfg = EditorWidgetConfig::new()
            .with_line_numbers(false)
            .with_minimap(false)
            .with_tab_size(2)
            .with_word_wrap(true)
            .with_readonly(true);
        assert!(!cfg.show_line_numbers);
        assert!(!cfg.show_minimap);
        assert_eq!(cfg.tab_size, 2);
        assert!(cfg.word_wrap);
        assert!(cfg.readonly);
        assert!(!cfg.is_default());
    }

    #[test]
    fn editor_widget_config_tab_size_clamped() {
        let cfg = EditorWidgetConfig::new().with_tab_size(0);
        assert_eq!(cfg.tab_size, 1);
        let cfg2 = EditorWidgetConfig::new().with_tab_size(20);
        assert_eq!(cfg2.tab_size, 8);
    }

    #[test]
    fn widget_selection_empty_and_contains() {
        let cursor = WidgetSelection::cursor(5, 10);
        assert!(cursor.is_empty());
        assert_eq!(cursor.line_span(), 1);
        assert!(cursor.contains_position(5, 10));
        assert!(!cursor.contains_position(5, 11));

        let sel = WidgetSelection::new(2, 1, 4, 5);
        assert!(!sel.is_empty());
        assert_eq!(sel.line_span(), 3);
        assert!(sel.contains_position(3, 3));
        assert!(!sel.contains_position(1, 1));
    }

    #[test]
    fn widget_selection_overlaps() {
        let a = WidgetSelection::new(1, 1, 3, 5);
        let b = WidgetSelection::new(3, 3, 5, 1);
        assert!(a.overlaps(&b));

        let c = WidgetSelection::new(4, 1, 6, 1);
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn widget_selection_set_merge() {
        let mut set = WidgetSelectionSet::new();
        set.add(WidgetSelection::new(1, 1, 3, 5));
        set.add(WidgetSelection::new(3, 3, 5, 1));
        set.add(WidgetSelection::new(10, 1, 12, 1));
        assert_eq!(set.len(), 3);
        set.merge_overlapping();
        assert_eq!(set.len(), 2);
    }

    // ---- VisibleRange tests ----

    #[test]
    fn visible_range_basic() {
        let vr = VisibleRange::new(5, 24, 100);
        assert_eq!(vr.first_visible_line(), 6);
        assert_eq!(vr.last_visible_line(), 29);
        assert!(!vr.is_at_top());
        assert!(!vr.is_at_bottom());
    }

    #[test]
    fn visible_range_at_top() {
        let vr = VisibleRange::new(0, 24, 100);
        assert!(vr.is_at_top());
    }

    #[test]
    fn visible_range_at_bottom() {
        let vr = VisibleRange::new(76, 24, 100);
        assert!(vr.is_at_bottom());
    }

    #[test]
    fn visible_range_contains_line() {
        let vr = VisibleRange::new(10, 20, 100);
        assert!(vr.contains_line(11)); // 1-based line 11 = 0-based 10
        assert!(vr.contains_line(30)); // 1-based line 30 = 0-based 29
        assert!(!vr.contains_line(31)); // 0-based 30 >= 10+20
        assert!(!vr.contains_line(5));
    }

    #[test]
    fn visible_range_visible_percentage() {
        let vr = VisibleRange::new(0, 50, 100);
        let pct = vr.visible_percentage();
        assert!((pct - 50.0).abs() < 0.01);
    }

    // ---- BracketMatcher tests ----

    #[test]
    fn bracket_matcher_find_opening_paren() {
        let result = BracketMatcher::find_matching_bracket("(hello)", 1);
        assert_eq!(result, Some(7));
    }

    #[test]
    fn bracket_matcher_find_closing_paren() {
        let result = BracketMatcher::find_matching_bracket("(hello)", 7);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn bracket_matcher_nested_brackets() {
        let result = BracketMatcher::find_matching_bracket("{a[b(c)d]e}", 1);
        assert_eq!(result, Some(11));
        let result2 = BracketMatcher::find_matching_bracket("{a[b(c)d]e}", 3);
        assert_eq!(result2, Some(9));
        let result3 = BracketMatcher::find_matching_bracket("{a[b(c)d]e}", 5);
        assert_eq!(result3, Some(7));
    }

    #[test]
    fn bracket_matcher_no_match() {
        let result = BracketMatcher::find_matching_bracket("(abc", 1);
        assert_eq!(result, None);
    }

    #[test]
    fn bracket_matcher_not_on_bracket() {
        let result = BracketMatcher::find_matching_bracket("abc", 2);
        assert_eq!(result, None);
    }

    // ---- GutterInfo tests ----

    #[test]
    fn gutter_info_basic() {
        let gi = GutterInfo::compute(100, true, false, false);
        assert!(gi.line_number_width > 0);
        assert_eq!(gi.fold_marker_width, 0);
        assert_eq!(gi.breakpoint_width, 0);
        assert!(gi.total_width() > 0);
    }

    #[test]
    fn gutter_info_all_features() {
        let gi = GutterInfo::compute(1000, true, true, true);
        assert!(gi.line_number_width > 0);
        assert_eq!(gi.fold_marker_width, 1);
        assert_eq!(gi.breakpoint_width, 2);
        let total = gi.line_number_width + 1 + 2;
        assert_eq!(gi.total_width(), total);
    }

    #[test]
    fn gutter_info_no_line_numbers() {
        let gi = GutterInfo::compute(100, false, false, false);
        assert_eq!(gi.line_number_width, 0);
        assert_eq!(gi.total_width(), 0);
    }

    // ---- MinimapData tests ----

    #[test]
    fn minimap_data_generation() {
        let lines = vec![
            "fn main() {".to_string(),
            "    println!(\"hello\");".to_string(),
            "}".to_string(),
        ];
        let data = MinimapData::from_lines(&lines, 20);
        assert_eq!(data.entries.len(), 3);
        assert!(data.entries[0].density > 0.0);
    }

    #[test]
    fn minimap_data_empty() {
        let lines: Vec<String> = vec![];
        let data = MinimapData::from_lines(&lines, 20);
        assert!(data.entries.is_empty());
    }

    // ---- IndentInfo tests ----

    #[test]
    fn indent_info_spaces() {
        let info = IndentInfo::from_line("    hello");
        assert_eq!(info.level, 4);
        assert!(!info.uses_tabs);
        assert_eq!(info.visual_width(4), 4);
    }

    #[test]
    fn indent_info_tabs() {
        let info = IndentInfo::from_line("\t\thello");
        assert_eq!(info.level, 2);
        assert!(info.uses_tabs);
        assert_eq!(info.visual_width(4), 8);
    }

    #[test]
    fn indent_info_mixed() {
        let info = IndentInfo::from_line("\t  hello");
        assert!(info.uses_tabs);
        assert_eq!(info.visual_width(4), 6);
    }

    #[test]
    fn indent_info_no_indent() {
        let info = IndentInfo::from_line("hello");
        assert_eq!(info.level, 0);
        assert!(!info.uses_tabs);
        assert_eq!(info.visual_width(4), 0);
    }

    // ---- ViewportLineMap tests ----

    #[test]
    fn viewport_line_map_basic() {
        let map = ViewportLineMap::new(10, 5, 100);
        assert_eq!(map.viewport_to_model(0), Some(11));
        assert_eq!(map.viewport_to_model(4), Some(15));
        assert_eq!(map.viewport_to_model(5), None);
    }

    #[test]
    fn viewport_line_map_model_to_viewport() {
        let map = ViewportLineMap::new(10, 5, 100);
        assert_eq!(map.model_to_viewport(11), Some(0));
        assert_eq!(map.model_to_viewport(15), Some(4));
        assert_eq!(map.model_to_viewport(16), None);
        assert_eq!(map.model_to_viewport(5), None);
    }

    #[test]
    fn viewport_line_map_clamps_to_total() {
        let map = ViewportLineMap::new(95, 10, 100);
        assert_eq!(map.viewport_to_model(4), Some(100));
        assert_eq!(map.viewport_to_model(5), None);
    }

    #[test]
    fn editor_stats_tracking() {
        let mut stats = EditorStats::new();
        stats.record_keystroke();
        stats.record_keystroke();
        stats.record_edit();
        stats.record_selection_change();
        assert_eq!(stats.total_actions(), 4);
        let display = format!("{stats}");
        assert!(display.contains("keys=2"));
        stats.reset();
        assert_eq!(stats.total_actions(), 0);
    }

#[test]
    fn widgetresizehandler_severity_ordering() {
        assert!(WidgetResizeHandlerSeverity::Critical > WidgetResizeHandlerSeverity::High);
        assert!(WidgetResizeHandlerSeverity::High > WidgetResizeHandlerSeverity::Medium);
        assert!(WidgetResizeHandlerSeverity::Medium > WidgetResizeHandlerSeverity::Low);
    }

    #[test]
    fn widgetresizehandler_severity_display() {
        assert_eq!(WidgetResizeHandlerSeverity::Low.to_string(), "low");
        assert_eq!(WidgetResizeHandlerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn widgetresizehandler_entry_creation() {
        let e = WidgetResizeHandlerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, WidgetResizeHandlerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn widgetresizehandler_entry_builder() {
        let e = WidgetResizeHandlerEntry::new("e2", "Entry 2")
            .with_severity(WidgetResizeHandlerSeverity::High)
            .with_detail("some detail")
            .with_min_width(42);
        assert_eq!(e.severity, WidgetResizeHandlerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.min_width, 42);
    }

    #[test]
    fn widgetresizehandler_entry_enable_disable() {
        let mut e = WidgetResizeHandlerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn widgetresizehandler_add_and_count() {
        let mut mgr = WidgetResizeHandler::new("test");
        mgr.add(WidgetResizeHandlerEntry::new("a", "A"));
        mgr.add(WidgetResizeHandlerEntry::new("b", "B").with_severity(WidgetResizeHandlerSeverity::High));
        assert_eq!(mgr.min_width(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn widgetresizehandler_remove() {
        let mut mgr = WidgetResizeHandler::new("test");
        mgr.add(WidgetResizeHandlerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn widgetresizehandler_capacity() {
        let mut mgr = WidgetResizeHandler::new("test").with_capacity(1);
        assert!(mgr.add(WidgetResizeHandlerEntry::new("a", "A")));
        assert!(!mgr.add(WidgetResizeHandlerEntry::new("b", "B")));
    }

    #[test]
    fn widgetresizehandler_sorted_by_severity() {
        let mut mgr = WidgetResizeHandler::new("test");
        mgr.add(WidgetResizeHandlerEntry::new("lo", "Low"));
        mgr.add(WidgetResizeHandlerEntry::new("hi", "High").with_severity(WidgetResizeHandlerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, WidgetResizeHandlerSeverity::Critical);
    }

    #[test]
    fn widgetresizehandler_summary() {
        let mgr = WidgetResizeHandler::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn widgetfocuscyclemanager_config_defaults() {
        let cfg = WidgetFocusCycleManagerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn widgetfocuscyclemanager_item_creation() {
        let item = WidgetFocusCycleManagerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn widgetfocuscyclemanager_add_and_get() {
        let mut mgr = WidgetFocusCycleManager::new(WidgetFocusCycleManagerConfig::new("test"));
        mgr.add(WidgetFocusCycleManagerItem::new("k1", "v1"));
        assert_eq!(mgr.widget_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn widgetfocuscyclemanager_remove_item() {
        let mut mgr = WidgetFocusCycleManager::new(WidgetFocusCycleManagerConfig::new("test"));
        mgr.add(WidgetFocusCycleManagerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn widgetfocuscyclemanager_sorted_by_priority() {
        let mut mgr = WidgetFocusCycleManager::new(WidgetFocusCycleManagerConfig::new("test"));
        mgr.add(WidgetFocusCycleManagerItem::new("lo", "low").with_priority(1));
        mgr.add(WidgetFocusCycleManagerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn widgetfocuscyclemanager_items_with_tag() {
        let mut mgr = WidgetFocusCycleManager::new(WidgetFocusCycleManagerConfig::new("test"));
        mgr.add(WidgetFocusCycleManagerItem::new("a", "1").with_tag("x"));
        mgr.add(WidgetFocusCycleManagerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn widgetfocuscyclemanager_report() {
        let mgr = WidgetFocusCycleManager::new(WidgetFocusCycleManagerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn widget_overlay_item_creation() {
        let e = WidgetOverlayItem::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn widget_overlay_item_with_priority() {
        let e = WidgetOverlayItem::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn widget_overlay_item_metadata() {
        let e = WidgetOverlayItem::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn widget_overlay_item_remove_meta() {
        let mut e = WidgetOverlayItem::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn widget_overlay_item_activate_deactivate() {
        let mut e = WidgetOverlayItem::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn widget_overlay_registry_add_sorted() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("lo", "Lo").with_priority(1));
        c.add(WidgetOverlayItem::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn widget_overlay_registry_capacity() {
        let mut c = WidgetOverlayRegistry::new(1);
        assert!(c.add(WidgetOverlayItem::new("a", "A")));
        assert!(!c.add(WidgetOverlayItem::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn widget_overlay_registry_remove() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn widget_overlay_registry_get() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn widget_overlay_registry_active_entries() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        c.add(WidgetOverlayItem::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn widget_overlay_registry_enable_disable() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn widget_overlay_registry_clear() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn widget_overlay_registry_find_by_label() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn widget_overlay_registry_top_n() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A").with_priority(1));
        c.add(WidgetOverlayItem::new("b", "B").with_priority(2));
        c.add(WidgetOverlayItem::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn widget_overlay_registry_deactivate_activate_all() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        c.add(WidgetOverlayItem::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn widget_overlay_registry_highest_priority() {
        let mut c = WidgetOverlayRegistry::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WidgetOverlayItem::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn widget_overlay_registry_contains() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn widget_overlay_registry_drain_inactive() {
        let mut c = WidgetOverlayRegistry::new(10);
        c.add(WidgetOverlayItem::new("a", "A"));
        c.add(WidgetOverlayItem::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xa_metrics_empty() {
        let m = XaMetrics::new("editor_wgt");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xa_metrics_record_and_mean() {
        let mut m = XaMetrics::new("editor_wgt");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xa_metrics_min_max() {
        let mut m = XaMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xa_metrics_variance_and_std() {
        let mut m = XaMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xa_metrics_percentile() {
        let mut m = XaMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xa_metrics_merge() {
        let mut a = XaMetrics::new("a");
        a.record(1.0);
        let mut b = XaMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xa_metrics_reset() {
        let mut m = XaMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xa_rate_window_empty() {
        let rw = XaRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xa_rate_window_tick_and_rate() {
        let mut rw = XaRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xa_lru_cache_basic() {
        let mut c = XaLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xa_lru_cache_contains_and_keys() {
        let mut c = XaLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xa_lru_cache_remove() {
        let mut c = XaLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xa_metrics_sum() {
        let mut m = XaMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xa_metrics_label() {
        let m = XaMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xa_lru_cache_clear() {
        let mut c = XaLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_17_push_and_len() {
        let mut rb = super::XbRingBuffer17::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_17_overwrite() {
        let mut rb = super::XbRingBuffer17::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_17_get_out_of_bounds() {
        let rb = super::XbRingBuffer17::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_17_drain_all() {
        let mut rb = super::XbRingBuffer17::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_17_peek_front_back() {
        let mut rb = super::XbRingBuffer17::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_17_clear() {
        let mut rb = super::XbRingBuffer17::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_17_capacity() {
        let rb = super::XbRingBuffer17::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_17_basic() {
        let h = super::xb_fnv1a_17(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_17(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_17_different_inputs() {
        let h1 = super::xb_fnv1a_17(b"abc");
        let h2 = super::xb_fnv1a_17(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_17_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_17(&data);
        let dec = super::xb_rle_decode_17(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_17_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_17(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_17(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_17_values() {
        assert!((super::xb_clamp_17(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_17(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_17(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_17_values() {
        assert!((super::xb_lerp_17(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_17(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_17(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_17_wrap_around_twice() {
        let mut rb = super::XbRingBuffer17::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 41 ----

    #[test]
    fn xc_41_pool_new_empty() {
        let pool: super::Xc41Pool<i32> = super::Xc41Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_41_pool_release_acquire() {
        let mut pool = super::Xc41Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_41_pool_acquire_empty() {
        let mut pool: super::Xc41Pool<i32> = super::Xc41Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_41_pool_full() {
        let mut pool = super::Xc41Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_41_pool_drain() {
        let mut pool = super::Xc41Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_41_pool_stats() {
        let mut pool = super::Xc41Pool::new(8);
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
    fn xc_41_pool_clear() {
        let mut pool = super::Xc41Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_41_pool_shrink() {
        let mut pool = super::Xc41Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_41_pool_default() {
        let pool: super::Xc41Pool<String> = super::Xc41Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_41_pool_extend() {
        let mut pool = super::Xc41Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_41_pool_retain() {
        let mut pool = super::Xc41Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_41_scheduler_round_robin() {
        let mut sched = super::Xc41Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_41_scheduler_empty() {
        let mut sched = super::Xc41Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_41_scheduler_reset() {
        let mut sched = super::Xc41Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_41_scheduler_add_remove() {
        let mut sched = super::Xc41Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_41_scheduler_targets() {
        let sched = super::Xc41Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_41_hash_empty() {
        assert_eq!(super::xc_41_hash(b""), 5381);
    }

    #[test]
    fn xc_41_hash_data() {
        let h = super::xc_41_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_41_hash(b"hello"), h);
    }

    #[test]
    fn xc_41_reverse_str() {
        assert_eq!(super::xc_41_reverse("abc"), "cba");
        assert_eq!(super::xc_41_reverse(""), "");
    }


    #[test]
    fn xe_29_pipeline_empty() {
        let p = super::Xe29Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_29_pipeline_parse_stage() {
        let p = super::Xe29Pipeline::new()
            .add_parse(super::xe_29_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_29_pipeline_transform_double() {
        let p = super::Xe29Pipeline::new()
            .add_transform(super::xe_29_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_29_pipeline_validate_reverse() {
        let p = super::Xe29Pipeline::new()
            .add_validate(super::xe_29_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_29_pipeline_emit_filter() {
        let p = super::Xe29Pipeline::new()
            .add_emit(super::xe_29_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_29_pipeline_multi_stage() {
        let p = super::Xe29Pipeline::new()
            .add_parse(super::xe_29_pipeline_identity)
            .add_transform(super::xe_29_pipeline_double)
            .add_validate(super::xe_29_pipeline_reverse)
            .add_emit(super::xe_29_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_29_pipeline_error_propagation() {
        let p = super::Xe29Pipeline::new()
            .add_parse(super::xe_29_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe29Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_29_pipeline_compose() {
        let p1 = super::Xe29Pipeline::new()
            .add_parse(super::xe_29_pipeline_identity);
        let p2 = super::Xe29Pipeline::new()
            .add_transform(super::xe_29_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_29_pipeline_error_display() {
        let e = super::Xe29PipelineError {
            stage: super::Xe29Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_29_cache_put_get() {
        let mut c = super::Xe29Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_29_cache_miss() {
        let mut c: super::Xe29Cache<&str, i32> = super::Xe29Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_29_cache_ttl_expiry() {
        let mut c = super::Xe29Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_29_cache_evict() {
        let mut c = super::Xe29Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_29_cache_capacity() {
        let mut c = super::Xe29Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_29_cache_stats() {
        let mut c = super::Xe29Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_29_cache_clear() {
        let mut c = super::Xe29Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #115 --

    #[test]
    fn xf115_trie_insert_search() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf115_trie_starts_with() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf115_trie_remove() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf115_trie_word_count() {
        let mut t = Xf115Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf115_trie_longest_prefix() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf115_trie_all_words() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf115_trie_autocomplete() {
        let mut t = Xf115Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf115_trie_empty_search() {
        let t = Xf115Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf115_bloom_add_contains() {
        let mut bf = Xf115BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf115_bloom_probably_absent() {
        let bf = Xf115BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf115_bloom_false_positive_rate() {
        let mut bf = Xf115BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf115_bloom_clear() {
        let mut bf = Xf115BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf115_bloom_union() {
        let mut a = Xf115BloomFilter::xf_new(512, 2);
        let mut b = Xf115BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf115_bloom_intersection_estimate() {
        let mut a = Xf115BloomFilter::xf_new(512, 2);
        let mut b = Xf115BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf115_bloom_union_size_mismatch() {
        let a = Xf115BloomFilter::xf_new(256, 2);
        let b = Xf115BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh40_skip_insert_contains() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh40_skip_remove() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh40_skip_len() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh40_skip_range_query() {
        let mut sl = super::Xh40SkipList::xh_new(4);
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
    fn xh40_skip_floor_ceiling() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh40_skip_rank() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh40_skip_empty() {
        let sl = super::Xh40SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh40_skip_duplicates() {
        let mut sl = super::Xh40SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh40_bitset_set_test() {
        let mut bs = super::Xh40BitSet::xh_new(256);
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
    fn xh40_bitset_clear_count() {
        let mut bs = super::Xh40BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh40_bitset_and_or_xor() {
        let mut a = super::Xh40BitSet::xh_new(128);
        let mut b = super::Xh40BitSet::xh_new(128);
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
    fn xh40_bitset_iter_ones() {
        let mut bs = super::Xh40BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh40_bitset_first_last() {
        let mut bs = super::Xh40BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh40_bitset_empty() {
        let bs = super::Xh40BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi40_deque_push_pop_back() {
        let mut dq = super::Xi40Deque::xi_new(4);
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
    fn xi40_deque_push_pop_front() {
        let mut dq = super::Xi40Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi40_deque_mixed_ops() {
        let mut dq = super::Xi40Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi40_deque_get_and_split() {
        let mut dq = super::Xi40Deque::xi_new(8);
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
    fn xi40_deque_rotate_left() {
        let mut dq = super::Xi40Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi40_deque_rotate_right() {
        let mut dq = super::Xi40Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi40_deque_grow() {
        let mut dq = super::Xi40Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi40_deque_empty() {
        let dq = super::Xi40Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi40_interval_tree_insert_query() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi40Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi40Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi40_interval_tree_overlap() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi40Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi40Interval::xi_new(12, 20));
        let q = super::Xi40Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi40_interval_tree_remove() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi40Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi40_interval_tree_gaps() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi40Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi40Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi40Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi40Interval::xi_new(8, 10));
    }

    #[test]
    fn xi40_interval_tree_merge() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi40Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi40Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi40Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi40Interval::xi_new(10, 15));
    }

    #[test]
    fn xi40_interval_tree_all() {
        let mut tree = super::Xi40IntervalTree::xi_new();
        tree.xi_insert(super::Xi40Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi40Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi40_interval_tree_empty() {
        let tree = super::Xi40IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi40_interval_tree_contains_point() {
        let iv = super::Xi40Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 40) ---

    #[test]
    fn xj_40_uf_make_and_find() {
        let mut uf = super::Xj40UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_40_uf_union_connected() {
        let mut uf = super::Xj40UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_40_uf_component_count() {
        let mut uf = super::Xj40UnionFind::xj_new();
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
    fn xj_40_uf_component_size() {
        let mut uf = super::Xj40UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_40_uf_largest_component() {
        let mut uf = super::Xj40UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_40_uf_many_elements() {
        let mut uf = super::Xj40UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_40_uf_separate_components() {
        let mut uf = super::Xj40UnionFind::xj_new();
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
    fn xj_40_uf_path_compression() {
        let mut uf = super::Xj40UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_40_bt_insert_get() {
        let mut bt = super::Xj40BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_40_bt_contains_len() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_40_bt_replace() {
        let mut bt = super::Xj40BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_40_bt_remove() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_40_bt_keys_values() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_40_bt_range() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_40_bt_min_max() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_40_bt_many_inserts() {
        let mut bt = super::Xj40BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_40 segment tree tests ---

    #[test]
    fn xk_40_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_40_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk40SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_40_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_40_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_40_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_40_st_single_element() {
        let data = vec![42];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_40_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk40SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_40_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk40SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_40 disjoint intervals tests ---

    #[test]
    fn xk_40_di_add_and_count() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_40_di_merge_overlap() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_40_di_contains() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_40_di_remove() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_40_di_covered_length() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_40_di_gaps() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_40_di_merge_adjacent() {
        let mut di = super::Xk40DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_40_di_empty() {
        let di = super::Xk40DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_40_rope_new_empty() {
        let rope = super::Xl40Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_40_rope_from_str() {
        let rope = super::Xl40Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_40_rope_insert_at() {
        let mut rope = super::Xl40Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_40_rope_delete_range() {
        let mut rope = super::Xl40Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_40_rope_char_at() {
        let rope = super::Xl40Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_40_rope_split_concat() {
        let rope = super::Xl40Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_40_rope_line_count() {
        let rope = super::Xl40Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_40_rope_line_at() {
        let rope = super::Xl40Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_40_sa_build_and_search() {
        let sa = super::Xl40SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_40_sa_count() {
        let sa = super::Xl40SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_40_sa_longest_repeated() {
        let sa = super::Xl40SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_40_sa_all_positions() {
        let sa = super::Xl40SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_40_sa_len() {
        let sa = super::Xl40SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_40_sa_empty() {
        let sa = super::Xl40SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_40_rope_slice() {
        let rope = super::Xl40Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_40_sa_search_start() {
        let sa = super::Xl40SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_40_sparse_set_get() {
        let mut m = super::Xm40MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_40_sparse_row_col() {
        let mut m = super::Xm40MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_40_sparse_transpose() {
        let mut m = super::Xm40MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_40_sparse_multiply_vec() {
        let mut m = super::Xm40MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_40_sparse_nnz_density() {
        let mut m = super::Xm40MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_40_sparse_clear() {
        let mut m = super::Xm40MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_40_sparse_overwrite_zero() {
        let mut m = super::Xm40MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_40_tokenizer_basic() {
        let t = super::Xm40Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_40_tokenizer_count() {
        let t = super::Xm40Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_40_tokenizer_unique() {
        let t = super::Xm40Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_40_tokenizer_frequency() {
        let t = super::Xm40Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_40_tokenizer_delimiter() {
        let t = super::Xm40Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_40_tokenizer_whitespace() {
        let t = super::Xm40Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_40_tokenizer_empty() {
        let t = super::Xm40Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}
