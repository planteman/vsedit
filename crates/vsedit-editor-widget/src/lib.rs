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
}
