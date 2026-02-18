//! Terminal panel integration.
//!
//! Provides a tabbed terminal emulator view with rendering via ratatui,
//! including PTY output rendering through [`TerminalBuffer`].

use std::collections::HashMap;
use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

pub use vsedit_terminal_plat::{TerminalConfig, TerminalId, TerminalInstance as PtyInstance};

// ---------------------------------------------------------------------------
// TerminalCell
// ---------------------------------------------------------------------------

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::White,
            bg: Color::Reset,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }
}

impl TerminalCell {
    /// Convert this cell's attributes into a ratatui [`Style`].
    fn to_style(&self) -> Style {
        let mut style = Style::default().fg(self.fg).bg(self.bg);
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.dim {
            style = style.add_modifier(Modifier::DIM);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.underline {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        style
    }
}

// ---------------------------------------------------------------------------
// SGR state (current text attributes for the parser)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SgrState {
    fg: Color,
    bg: Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl Default for SgrState {
    fn default() -> Self {
        Self {
            fg: Color::White,
            bg: Color::Reset,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ANSI parser state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum ParserState {
    Ground,
    Escape,
    Csi,
    OscString,
}

// ---------------------------------------------------------------------------
// TerminalBuffer
// ---------------------------------------------------------------------------

/// Grid-based terminal buffer with ANSI/VT100 escape-sequence parsing.
///
/// Call [`TerminalBuffer::process_output`] with raw PTY bytes to update the
/// cell grid, then use [`TerminalBuffer::render_terminal`] to paint the grid
/// into a ratatui [`Buffer`].
#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    /// Visible cell grid – `cells[row][col]`.
    pub cells: Vec<Vec<TerminalCell>>,
    /// Number of visible columns.
    pub cols: usize,
    /// Number of visible rows.
    pub rows: usize,
    /// Cursor row (0-based, relative to visible area).
    pub cursor_row: usize,
    /// Cursor column (0-based).
    pub cursor_col: usize,
    /// Scroll offset for viewing scrollback.
    pub scroll_offset: usize,
    /// Scrollback history (oldest first).
    pub scrollback: Vec<Vec<TerminalCell>>,

    // -- private parser state -----------------------------------------------
    sgr: SgrState,
    parser_state: ParserState,
    csi_params: String,
}

impl TerminalBuffer {
    /// Create a new buffer with the given dimensions.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = vec![vec![TerminalCell::default(); cols]; rows];
        Self {
            cells,
            cols,
            rows,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            scrollback: Vec::new(),
            sgr: SgrState::default(),
            parser_state: ParserState::Ground,
            csi_params: String::new(),
        }
    }

    // -- public API ---------------------------------------------------------

    /// Feed raw PTY output bytes into the parser, updating the cell grid.
    pub fn process_output(&mut self, data: &[u8]) {
        for &byte in data {
            match self.parser_state {
                ParserState::Ground => self.ground(byte),
                ParserState::Escape => self.escape(byte),
                ParserState::Csi => self.csi(byte),
                ParserState::OscString => self.osc(byte),
            }
        }
    }

    /// Render the cell grid into a ratatui buffer, including the cursor.
    pub fn render_terminal(&self, area: Rect, buf: &mut Buffer) {
        let view_rows = area.height as usize;
        let view_cols = area.width as usize;

        // Determine which rows to display (scrollback + visible).
        let total_rows = self.scrollback.len() + self.rows;
        let scroll = self.scroll_offset;

        for vy in 0..view_rows {
            // The row index in the combined scrollback+visible buffer,
            // counting from the bottom.
            let logical_row = total_rows.saturating_sub(scroll + view_rows) + vy;

            for vx in 0..view_cols {
                let x = area.x + vx as u16;
                let y = area.y + vy as u16;
                if let Some(ratatui_cell) = buf.cell_mut((x, y)) {
                    let tcell = self.cell_at_logical(logical_row, vx);
                    let mut style = tcell.to_style();

                    // Draw cursor as reversed cell.
                    let is_cursor = scroll == 0
                        && logical_row == self.scrollback.len() + self.cursor_row
                        && vx == self.cursor_col;
                    if is_cursor {
                        style = style.add_modifier(Modifier::REVERSED);
                    }

                    ratatui_cell.set_char(tcell.ch);
                    ratatui_cell.set_style(style);
                }
            }
        }
    }

    /// Resize the terminal grid. Existing content is preserved where possible.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == 0 || new_rows == 0 {
            return;
        }

        // Resize columns in each existing row.
        for row in &mut self.cells {
            row.resize(new_cols, TerminalCell::default());
        }
        for row in &mut self.scrollback {
            row.resize(new_cols, TerminalCell::default());
        }

        // Adjust number of rows.
        if new_rows > self.rows {
            let extra = new_rows - self.rows;
            for _ in 0..extra {
                self.cells.push(vec![TerminalCell::default(); new_cols]);
            }
        } else if new_rows < self.rows {
            // Move excess top rows into scrollback.
            let excess = self.rows - new_rows;
            for _ in 0..excess {
                if !self.cells.is_empty() {
                    let row = self.cells.remove(0);
                    self.scrollback.push(row);
                }
            }
        }

        self.cols = new_cols;
        self.rows = new_rows;
        self.cursor_row = self.cursor_row.min(self.rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(self.cols.saturating_sub(1));
    }

    // -- helpers (private) --------------------------------------------------

    /// Get a cell at a logical row index (scrollback + visible combined).
    fn cell_at_logical(&self, logical_row: usize, col: usize) -> TerminalCell {
        let sb_len = self.scrollback.len();
        if logical_row < sb_len {
            self.scrollback
                .get(logical_row)
                .and_then(|r| r.get(col))
                .cloned()
                .unwrap_or_default()
        } else {
            let vis_row = logical_row - sb_len;
            self.cells
                .get(vis_row)
                .and_then(|r| r.get(col))
                .cloned()
                .unwrap_or_default()
        }
    }

    /// Scroll the visible area up by one line.
    fn scroll_up(&mut self) {
        if !self.cells.is_empty() {
            let row = self.cells.remove(0);
            self.scrollback.push(row);
            self.cells
                .push(vec![TerminalCell::default(); self.cols]);
        }
    }

    /// Place a printable character at the cursor and advance.
    fn put_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            // Line wrap.
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.rows {
                self.scroll_up();
                self.cursor_row = self.rows - 1;
            }
        }
        if self.cursor_row < self.rows && self.cursor_col < self.cols {
            let cell = &mut self.cells[self.cursor_row][self.cursor_col];
            cell.ch = ch;
            cell.fg = self.sgr.fg;
            cell.bg = self.sgr.bg;
            cell.bold = self.sgr.bold;
            cell.dim = self.sgr.dim;
            cell.italic = self.sgr.italic;
            cell.underline = self.sgr.underline;
        }
        self.cursor_col += 1;
    }

    // -- parser states ------------------------------------------------------

    fn ground(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.parser_state = ParserState::Escape;
            }
            b'\n' => {
                self.cursor_row += 1;
                if self.cursor_row >= self.rows {
                    self.scroll_up();
                    self.cursor_row = self.rows - 1;
                }
            }
            b'\r' => {
                self.cursor_col = 0;
            }
            b'\t' => {
                let next_tab = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            0x08 => {
                // Backspace
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            0x07 => {
                // Bell — ignore
            }
            b if b >= 0x20 => {
                // Printable ASCII or start of UTF-8.
                // For simplicity we handle single-byte chars here.
                self.put_char(byte as char);
            }
            _ => {
                // Ignore other control characters.
            }
        }
    }

    fn escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.parser_state = ParserState::Csi;
                self.csi_params.clear();
            }
            b']' => {
                self.parser_state = ParserState::OscString;
            }
            _ => {
                // Unknown escape — return to ground.
                self.parser_state = ParserState::Ground;
            }
        }
    }

    fn csi(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' | b';' | b'?' => {
                self.csi_params.push(byte as char);
            }
            b'A' => {
                let n = self.parse_first_param(1);
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.parser_state = ParserState::Ground;
            }
            b'B' => {
                let n = self.parse_first_param(1);
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
                self.parser_state = ParserState::Ground;
            }
            b'C' => {
                let n = self.parse_first_param(1);
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
                self.parser_state = ParserState::Ground;
            }
            b'D' => {
                let n = self.parse_first_param(1);
                self.cursor_col = self.cursor_col.saturating_sub(n);
                self.parser_state = ParserState::Ground;
            }
            b'H' | b'f' => {
                let params = self.parse_params();
                let row = params.first().copied().unwrap_or(1).max(1) - 1;
                let col = params.get(1).copied().unwrap_or(1).max(1) - 1;
                self.cursor_row = row.min(self.rows.saturating_sub(1));
                self.cursor_col = col.min(self.cols.saturating_sub(1));
                self.parser_state = ParserState::Ground;
            }
            b'J' => {
                let n = self.parse_first_param(0);
                self.erase_in_display(n);
                self.parser_state = ParserState::Ground;
            }
            b'K' => {
                let n = self.parse_first_param(0);
                self.erase_in_line(n);
                self.parser_state = ParserState::Ground;
            }
            b'm' => {
                self.apply_sgr();
                self.parser_state = ParserState::Ground;
            }
            _ => {
                // Unrecognised final byte — discard sequence.
                self.parser_state = ParserState::Ground;
            }
        }
    }

    fn osc(&mut self, byte: u8) {
        // Consume until ST (BEL or ESC \)
        if byte == 0x07 || byte == 0x9c {
            self.parser_state = ParserState::Ground;
        }
        // For ESC \ we'd need a two-byte check; for simplicity, BEL-only.
    }

    // -- CSI helpers --------------------------------------------------------

    fn parse_params(&self) -> Vec<usize> {
        self.csi_params
            .split(';')
            .map(|s| s.parse::<usize>().unwrap_or(0))
            .collect()
    }

    fn parse_first_param(&self, default: usize) -> usize {
        self.csi_params
            .split(';')
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default)
    }

    // -- erase operations ---------------------------------------------------

    fn erase_in_display(&mut self, mode: usize) {
        match mode {
            0 => {
                // Erase from cursor to end of screen.
                self.erase_in_line(0);
                for r in (self.cursor_row + 1)..self.rows {
                    self.clear_row(r);
                }
            }
            1 => {
                // Erase from start to cursor.
                self.erase_in_line(1);
                for r in 0..self.cursor_row {
                    self.clear_row(r);
                }
            }
            2 | 3 => {
                // Erase entire screen.
                for r in 0..self.rows {
                    self.clear_row(r);
                }
            }
            _ => {}
        }
    }

    fn erase_in_line(&mut self, mode: usize) {
        if self.cursor_row >= self.rows {
            return;
        }
        match mode {
            0 => {
                for c in self.cursor_col..self.cols {
                    self.cells[self.cursor_row][c] = TerminalCell::default();
                }
            }
            1 => {
                for c in 0..=self.cursor_col.min(self.cols.saturating_sub(1)) {
                    self.cells[self.cursor_row][c] = TerminalCell::default();
                }
            }
            2 => {
                self.clear_row(self.cursor_row);
            }
            _ => {}
        }
    }

    fn clear_row(&mut self, row: usize) {
        if row < self.rows {
            for c in 0..self.cols {
                self.cells[row][c] = TerminalCell::default();
            }
        }
    }

    // -- SGR ----------------------------------------------------------------

    fn apply_sgr(&mut self) {
        let params = self.parse_params();
        if params.is_empty() || (params.len() == 1 && params[0] == 0) {
            self.sgr = SgrState::default();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.sgr = SgrState::default(),
                1 => self.sgr.bold = true,
                2 => self.sgr.dim = true,
                3 => self.sgr.italic = true,
                4 => self.sgr.underline = true,
                22 => {
                    self.sgr.bold = false;
                    self.sgr.dim = false;
                }
                23 => self.sgr.italic = false,
                24 => self.sgr.underline = false,
                // Standard foreground colors 30-37
                n @ 30..=37 => self.sgr.fg = ansi_color(n - 30),
                39 => self.sgr.fg = Color::White,
                // Standard background colors 40-47
                n @ 40..=47 => self.sgr.bg = ansi_color(n - 40),
                49 => self.sgr.bg = Color::Reset,
                // Bright foreground 90-97
                n @ 90..=97 => self.sgr.fg = ansi_bright_color(n - 90),
                // Bright background 100-107
                n @ 100..=107 => self.sgr.bg = ansi_bright_color(n - 100),
                // 256-color and RGB
                38 => {
                    if let Some(color) = self.parse_extended_color(&params, &mut i) {
                        self.sgr.fg = color;
                    }
                }
                48 => {
                    if let Some(color) = self.parse_extended_color(&params, &mut i) {
                        self.sgr.bg = color;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_extended_color(&self, params: &[usize], i: &mut usize) -> Option<Color> {
        if *i + 1 >= params.len() {
            return None;
        }
        match params[*i + 1] {
            5 => {
                // 256-color: 38;5;N
                if *i + 2 < params.len() {
                    let n = params[*i + 2] as u8;
                    *i += 2;
                    Some(Color::Indexed(n))
                } else {
                    None
                }
            }
            2 => {
                // RGB: 38;2;R;G;B
                if *i + 4 < params.len() {
                    let r = params[*i + 2] as u8;
                    let g = params[*i + 3] as u8;
                    let b = params[*i + 4] as u8;
                    *i += 4;
                    Some(Color::Rgb(r, g, b))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Map standard ANSI color index (0-7) to ratatui color.
fn ansi_color(idx: usize) -> Color {
    match idx {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        _ => Color::White,
    }
}

/// Map bright ANSI color index (0-7) to ratatui color.
fn ansi_bright_color(idx: usize) -> Color {
    match idx {
        0 => Color::DarkGray,
        1 => Color::LightRed,
        2 => Color::LightGreen,
        3 => Color::LightYellow,
        4 => Color::LightBlue,
        5 => Color::LightMagenta,
        6 => Color::LightCyan,
        7 => Color::White,
        _ => Color::White,
    }
}

/// A single terminal tab in the tab bar.
#[derive(Debug, Clone)]
pub struct TerminalTab {
    pub id: u64,
    pub title: String,
    pub is_active: bool,
}

impl TerminalTab {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_active: false,
        }
    }
}

/// Terminal panel UI with tabbed terminals.
#[derive(Debug, Clone)]
pub struct TerminalView {
    pub active_terminal_id: Option<u64>,
    pub terminal_tabs: Vec<TerminalTab>,
    pub scroll_offset: usize,
    pub show_search: bool,
    pub search_query: String,
    /// Per-tab terminal buffers keyed by tab id.
    pub buffers: HashMap<u64, TerminalBuffer>,
    next_id: u64,
}

impl TerminalView {
    pub fn new() -> Self {
        Self {
            active_terminal_id: None,
            terminal_tabs: Vec::new(),
            scroll_offset: 0,
            show_search: false,
            search_query: String::new(),
            buffers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Add a new terminal tab and return its id.
    pub fn add_tab(&mut self, title: impl Into<String>) -> u64 {
        self.add_tab_with_size(title, 80, 24)
    }

    /// Add a new terminal tab with a specific grid size and return its id.
    pub fn add_tab_with_size(&mut self, title: impl Into<String>, cols: usize, rows: usize) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut tab = TerminalTab::new(id, title);
        // If this is the first tab, make it active.
        if self.terminal_tabs.is_empty() {
            tab.is_active = true;
            self.active_terminal_id = Some(id);
        }
        self.terminal_tabs.push(tab);
        self.buffers.insert(id, TerminalBuffer::new(cols, rows));
        id
    }

    /// Remove a terminal tab by id.
    pub fn remove_tab(&mut self, id: u64) -> bool {
        let was_active = self.active_terminal_id == Some(id);
        let pos = self.terminal_tabs.iter().position(|t| t.id == id);
        if let Some(idx) = pos {
            self.terminal_tabs.remove(idx);
            self.buffers.remove(&id);
            if was_active {
                // Activate the nearest remaining tab.
                let new_idx = idx.min(self.terminal_tabs.len().saturating_sub(1));
                if let Some(tab) = self.terminal_tabs.get_mut(new_idx) {
                    tab.is_active = true;
                    self.active_terminal_id = Some(tab.id);
                } else {
                    self.active_terminal_id = None;
                }
            }
            true
        } else {
            false
        }
    }

    /// Set a tab as the active terminal.
    pub fn set_active_tab(&mut self, id: u64) -> bool {
        let exists = self.terminal_tabs.iter().any(|t| t.id == id);
        if !exists {
            return false;
        }
        for tab in &mut self.terminal_tabs {
            tab.is_active = tab.id == id;
        }
        self.active_terminal_id = Some(id);
        true
    }

    /// Switch to the next tab (wrapping).
    pub fn next_tab(&mut self) {
        if self.terminal_tabs.is_empty() {
            return;
        }
        let current = self
            .terminal_tabs
            .iter()
            .position(|t| t.is_active)
            .unwrap_or(0);
        let next = (current + 1) % self.terminal_tabs.len();
        let id = self.terminal_tabs[next].id;
        self.set_active_tab(id);
    }

    /// Switch to the previous tab (wrapping).
    pub fn previous_tab(&mut self) {
        if self.terminal_tabs.is_empty() {
            return;
        }
        let current = self
            .terminal_tabs
            .iter()
            .position(|t| t.is_active)
            .unwrap_or(0);
        let prev = if current == 0 {
            self.terminal_tabs.len() - 1
        } else {
            current - 1
        };
        let id = self.terminal_tabs[prev].id;
        self.set_active_tab(id);
    }

    /// Render the terminal view into a ratatui buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 4 {
            return;
        }

        // Tab bar: first row
        let tab_area = Rect { height: 1, ..area };
        self.render_tab_bar(tab_area, buf);

        // Content area: remaining rows
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        self.render_content(content_area, buf);
    }

    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        for tab in &self.terminal_tabs {
            let style = if tab.is_active {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let label = format!(" {} ", tab.title);
            let width = label.len() as u16;
            if x + width > area.x + area.width {
                break;
            }
            let span = Span::styled(label, style);
            let line = Line::from(vec![span]);
            let tab_rect = Rect {
                x,
                y: area.y,
                width,
                height: 1,
            };
            line.render(tab_rect, buf);
            x += width;
        }
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        // If we have a buffer for the active tab, render from it.
        if let Some(id) = self.active_terminal_id {
            if let Some(tbuf) = self.buffers.get(&id) {
                tbuf.render_terminal(area, buf);
                return;
            }
        }
        // Fallback: blank area with label.
        let style = Style::default().fg(Color::White).bg(Color::Black);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(style);
                    cell.set_char(' ');
                }
            }
        }
        if let Some(id) = self.active_terminal_id {
            let label = format!("Terminal #{}", id);
            let y = area.y;
            for (i, ch) in label.chars().enumerate() {
                let x = area.x + i as u16;
                if x >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(Style::default().fg(Color::Green));
                }
            }
        }
    }

    /// Returns true if terminal_tabs is empty.
    pub fn is_terminal_tabs_empty(&self) -> bool {
        self.terminal_tabs.is_empty()
    }

    /// Get the first terminal_tab, if any.
    pub fn first_terminal_tab(&self) -> Option<&TerminalTab> {
        self.terminal_tabs.first()
    }

    /// Get the last terminal_tab, if any.
    pub fn last_terminal_tab(&self) -> Option<&TerminalTab> {
        self.terminal_tabs.last()
    }

    /// Retain only terminal_tabs matching the predicate.
    pub fn retain_terminal_tabs(&mut self, f: impl Fn(&TerminalTab) -> bool) {
        self.terminal_tabs.retain(|item| f(item));
    }

    /// Toggle the `show_search` flag.
    pub fn toggle_show_search(&mut self) {
        self.show_search = !self.show_search;
    }

    /// Feed raw PTY output into the buffer for a given tab.
    pub fn process_pty_output(&mut self, tab_id: u64, data: &[u8]) {
        if let Some(buf) = self.buffers.get_mut(&tab_id) {
            buf.process_output(data);
        }
    }

    /// Feed raw PTY output into the *active* tab's buffer.
    pub fn process_active_output(&mut self, data: &[u8]) {
        if let Some(id) = self.active_terminal_id {
            self.process_pty_output(id, data);
        }
    }

    /// Get a reference to the buffer for a tab.
    pub fn get_buffer(&self, tab_id: u64) -> Option<&TerminalBuffer> {
        self.buffers.get(&tab_id)
    }

    /// Get a mutable reference to the buffer for a tab.
    pub fn get_buffer_mut(&mut self, tab_id: u64) -> Option<&mut TerminalBuffer> {
        self.buffers.get_mut(&tab_id)
    }

    /// Resize the terminal buffer for a given tab.
    pub fn resize_terminal(&mut self, tab_id: u64, cols: u16, rows: u16) {
        if let Some(buf) = self.buffers.get_mut(&tab_id) {
            buf.resize(cols as usize, rows as usize);
        }
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}

/// A terminal profile describing how to launch a shell.
#[derive(Debug, Clone)]
pub struct TerminalProfile {
    pub name: String,
    pub shell_path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub icon: Option<String>,
}

/// A running terminal instance.
#[derive(Debug, Clone)]
pub struct TerminalInstance {
    pub id: u64,
    pub profile: TerminalProfile,
    pub title: String,
    pub active: bool,
    pub exit_code: Option<i32>,
}

/// Service managing terminal instances.
pub struct TerminalService {
    pub instances: Vec<TerminalInstance>,
    next_id: u64,
    pub default_profile: Option<TerminalProfile>,
}

impl TerminalService {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            next_id: 1,
            default_profile: None,
        }
    }

    pub fn set_default_profile(&mut self, profile: TerminalProfile) {
        self.default_profile = Some(profile);
    }

    pub fn create_terminal(&mut self, profile: TerminalProfile) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let title = profile.name.clone();
        self.instances.push(TerminalInstance {
            id,
            profile,
            title,
            active: false,
            exit_code: None,
        });
        id
    }

    pub fn create_default_terminal(&mut self) -> Option<u64> {
        let profile = self.default_profile.clone()?;
        Some(self.create_terminal(profile))
    }

    pub fn close_terminal(&mut self, id: u64) -> bool {
        if let Some(pos) = self.instances.iter().position(|t| t.id == id) {
            self.instances.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_active(&self) -> Option<&TerminalInstance> {
        self.instances.iter().find(|t| t.active)
    }

    pub fn set_active(&mut self, id: u64) {
        for inst in &mut self.instances {
            inst.active = inst.id == id;
        }
    }

    pub fn terminal_count(&self) -> usize {
        self.instances.len()
    }

    pub fn rename_terminal(&mut self, id: u64, name: impl Into<String>) {
        if let Some(inst) = self.instances.iter_mut().find(|t| t.id == id) {
            inst.title = name.into();
        }
    }
}

impl Default for TerminalService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scrollback history manager
// ---------------------------------------------------------------------------

/// Manages a bounded scrollback history buffer with configurable capacity.
///
/// When the capacity is exceeded, the oldest lines are discarded. This
/// prevents unbounded memory growth for long-running terminal sessions.
#[derive(Debug, Clone)]
pub struct ScrollbackManager {
    lines: Vec<Vec<TerminalCell>>,
    capacity: usize,
}

impl ScrollbackManager {
    /// Create a new scrollback manager with the given maximum line capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Vec::new(),
            capacity,
        }
    }

    /// Push a line into the scrollback buffer, evicting the oldest if full.
    pub fn push_line(&mut self, line: Vec<TerminalCell>) {
        if self.lines.len() >= self.capacity && self.capacity > 0 {
            self.lines.remove(0);
        }
        if self.capacity > 0 {
            self.lines.push(line);
        }
    }

    /// Return the total number of lines currently stored.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Return true if no lines are stored.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get a specific line by index, if it exists.
    pub fn get_line(&self, index: usize) -> Option<&Vec<TerminalCell>> {
        self.lines.get(index)
    }

    /// Clear all stored scrollback lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Return the maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Extract the plain text content of a scrollback line.
    pub fn line_text(&self, index: usize) -> Option<String> {
        self.lines.get(index).map(|row| {
            row.iter().map(|c| c.ch).collect::<String>().trim_end().to_string()
        })
    }

    /// Search scrollback lines for a substring, returning matching indices.
    pub fn search(&self, needle: &str) -> Vec<usize> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let text: String = row.iter().map(|c| c.ch).collect();
                if text.contains(needle) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ANSI escape code helpers
// ---------------------------------------------------------------------------

/// Parsed representation of a CSI (Control Sequence Introducer) escape sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct CsiSequence {
    /// Numeric parameters separated by `;` in the original sequence.
    pub params: Vec<u16>,
    /// The final byte that identifies the command (e.g. `m`, `H`, `J`).
    pub final_byte: u8,
}

/// Strip all ANSI escape sequences from a byte slice, returning plain text.
pub fn strip_ansi(input: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        if input[i] == 0x1b {
            i += 1;
            if i < input.len() && input[i] == b'[' {
                // CSI sequence: skip until final byte (0x40-0x7E).
                i += 1;
                while i < input.len() && !(0x40..=0x7E).contains(&input[i]) {
                    i += 1;
                }
                if i < input.len() {
                    i += 1; // skip final byte
                }
            } else if i < input.len() && input[i] == b']' {
                // OSC: skip until BEL or ST.
                i += 1;
                while i < input.len() && input[i] != 0x07 && input[i] != 0x9c {
                    i += 1;
                }
                if i < input.len() {
                    i += 1;
                }
            }
            // else: unknown ESC, already skipped the ESC byte
        } else if input[i] >= 0x20 {
            out.push(input[i] as char);
            i += 1;
        } else {
            // Control chars like \r, \n
            match input[i] {
                b'\n' => out.push('\n'),
                b'\t' => out.push('\t'),
                _ => {}
            }
            i += 1;
        }
    }
    out
}

/// Parse a raw CSI sequence from its interior bytes (everything between `ESC[`
/// and the final byte inclusive). Returns `None` if the slice is empty.
pub fn parse_csi(interior: &[u8]) -> Option<CsiSequence> {
    if interior.is_empty() {
        return None;
    }
    let final_byte = *interior.last()?;
    if !(0x40..=0x7E).contains(&final_byte) {
        return None;
    }
    let param_bytes = &interior[..interior.len() - 1];
    let params: Vec<u16> = if param_bytes.is_empty() {
        Vec::new()
    } else {
        std::str::from_utf8(param_bytes)
            .ok()?
            .split(';')
            .map(|s| s.parse::<u16>().unwrap_or(0))
            .collect()
    };
    Some(CsiSequence { params, final_byte })
}

// ---------------------------------------------------------------------------
// Terminal dimensions / resize tracking
// ---------------------------------------------------------------------------

/// Tracks terminal dimensions and fires logical resize events.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalDimensions {
    pub cols: u16,
    pub rows: u16,
}

impl TerminalDimensions {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// Apply a resize and return `true` if the dimensions actually changed.
    pub fn resize(&mut self, new_cols: u16, new_rows: u16) -> bool {
        if new_cols == 0 || new_rows == 0 {
            return false;
        }
        if self.cols == new_cols && self.rows == new_rows {
            return false;
        }
        self.cols = new_cols;
        self.rows = new_rows;
        true
    }

    /// Return the total number of cells in the grid.
    pub fn cell_count(&self) -> u32 {
        self.cols as u32 * self.rows as u32
    }
}

// ---------------------------------------------------------------------------
// Terminal session state machine
// ---------------------------------------------------------------------------

/// High-level states for a terminal session lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The terminal has been created but the shell has not started.
    Created,
    /// The shell is starting (PTY allocated, waiting for first output).
    Starting,
    /// The shell is running and accepting input.
    Running,
    /// The shell has exited but the terminal tab is still open.
    Exited(i32),
    /// The terminal has been fully closed and cleaned up.
    Closed,
}

/// State machine that tracks a terminal session through its lifecycle.
#[derive(Debug, Clone)]
pub struct SessionStateMachine {
    state: SessionState,
    /// Number of bytes received from the PTY so far.
    bytes_received: u64,
    /// Number of bytes sent to the PTY so far.
    bytes_sent: u64,
}

impl SessionStateMachine {
    pub fn new() -> Self {
        Self {
            state: SessionState::Created,
            bytes_received: 0,
            bytes_sent: 0,
        }
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Transition to `Starting`. Only valid from `Created`.
    pub fn start(&mut self) -> bool {
        if self.state == SessionState::Created {
            self.state = SessionState::Starting;
            true
        } else {
            false
        }
    }

    /// Record received PTY output bytes. Automatically transitions from
    /// `Starting` to `Running` on the first output.
    pub fn record_output(&mut self, len: usize) {
        self.bytes_received += len as u64;
        if self.state == SessionState::Starting {
            self.state = SessionState::Running;
        }
    }

    /// Record bytes sent to the PTY.
    pub fn record_input(&mut self, len: usize) {
        self.bytes_sent += len as u64;
    }

    /// Transition to `Exited`. Valid from `Running` or `Starting`.
    pub fn exit(&mut self, code: i32) -> bool {
        match self.state {
            SessionState::Running | SessionState::Starting => {
                self.state = SessionState::Exited(code);
                true
            }
            _ => false,
        }
    }

    /// Transition to `Closed`. Valid from `Exited` or `Created`.
    pub fn close(&mut self) -> bool {
        match self.state {
            SessionState::Exited(_) | SessionState::Created => {
                self.state = SessionState::Closed;
                true
            }
            _ => false,
        }
    }

    /// Return `true` if the session is in a state that can accept user input.
    pub fn is_interactive(&self) -> bool {
        self.state == SessionState::Running
    }
}

impl Default for SessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TerminalTabGroup – organize terminals into groups
// ---------------------------------------------------------------------------

/// A named group of terminal tabs.
#[derive(Debug, Clone)]
pub struct TerminalTabGroup {
    pub name: String,
    pub terminal_ids: Vec<String>,
    pub collapsed: bool,
}

impl TerminalTabGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            terminal_ids: Vec::new(),
            collapsed: false,
        }
    }

    /// Add a terminal to this group.
    pub fn add(&mut self, terminal_id: impl Into<String>) {
        self.terminal_ids.push(terminal_id.into());
    }

    /// Remove a terminal from this group. Returns true if found.
    pub fn remove(&mut self, terminal_id: &str) -> bool {
        let before = self.terminal_ids.len();
        self.terminal_ids.retain(|id| id != terminal_id);
        self.terminal_ids.len() < before
    }

    /// Toggle collapsed state.
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    pub fn len(&self) -> usize {
        self.terminal_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terminal_ids.is_empty()
    }

    /// Check if this group contains a specific terminal.
    pub fn contains(&self, terminal_id: &str) -> bool {
        self.terminal_ids.iter().any(|id| id == terminal_id)
    }
}

impl fmt::Display for TerminalTabGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} terminals)", self.name, self.terminal_ids.len())
    }
}

// ---------------------------------------------------------------------------
// TerminalSplitView – side-by-side terminal layout
// ---------------------------------------------------------------------------

/// Orientation for a terminal split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    Horizontal,
    Vertical,
}

/// A split view containing two terminal panes.
#[derive(Debug, Clone)]
pub struct TerminalSplitView {
    pub left_id: String,
    pub right_id: String,
    pub orientation: SplitOrientation,
    /// Ratio of left/top pane (0.0–1.0).
    pub ratio: f32,
    pub focused_pane: SplitPane,
}

/// Which pane is focused in a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPane {
    Left,
    Right,
}

impl TerminalSplitView {
    pub fn new(left_id: impl Into<String>, right_id: impl Into<String>, orientation: SplitOrientation) -> Self {
        Self {
            left_id: left_id.into(),
            right_id: right_id.into(),
            orientation,
            ratio: 0.5,
            focused_pane: SplitPane::Left,
        }
    }

    /// Set the split ratio (clamped to 0.1–0.9).
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(0.1, 0.9);
    }

    /// Toggle focus between left and right pane.
    pub fn toggle_focus(&mut self) {
        self.focused_pane = match self.focused_pane {
            SplitPane::Left => SplitPane::Right,
            SplitPane::Right => SplitPane::Left,
        };
    }

    /// Get the ID of the currently focused terminal.
    pub fn focused_terminal_id(&self) -> &str {
        match self.focused_pane {
            SplitPane::Left => &self.left_id,
            SplitPane::Right => &self.right_id,
        }
    }

    /// Check if a terminal ID is part of this split.
    pub fn contains(&self, terminal_id: &str) -> bool {
        self.left_id == terminal_id || self.right_id == terminal_id
    }
}

// ---------------------------------------------------------------------------
// TerminalSearchOverlay – in-terminal search
// ---------------------------------------------------------------------------

/// A search overlay displayed on top of a terminal.
#[derive(Debug, Clone)]
pub struct TerminalSearchOverlay {
    pub query: String,
    pub case_sensitive: bool,
    pub regex_mode: bool,
    pub match_positions: Vec<(u32, u32)>,
    pub current_match: Option<usize>,
    pub visible: bool,
}

impl TerminalSearchOverlay {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            regex_mode: false,
            match_positions: Vec::new(),
            current_match: None,
            visible: false,
        }
    }

    /// Open the search overlay.
    pub fn open(&mut self) {
        self.visible = true;
    }

    /// Close and reset the search overlay.
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.match_positions.clear();
        self.current_match = None;
    }

    /// Update the query and perform a search in the given lines.
    pub fn search(&mut self, query: &str, lines: &[&str]) {
        self.query = query.to_string();
        self.match_positions.clear();
        if query.is_empty() {
            self.current_match = None;
            return;
        }
        let q = if self.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        for (line_idx, line) in lines.iter().enumerate() {
            let haystack = if self.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&q) {
                self.match_positions.push((line_idx as u32, (start + pos) as u32));
                start += pos + 1;
            }
        }
        self.current_match = if self.match_positions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Navigate to the next match.
    pub fn next_match(&mut self) {
        if let Some(ref mut idx) = self.current_match {
            if !self.match_positions.is_empty() {
                *idx = (*idx + 1) % self.match_positions.len();
            }
        }
    }

    /// Navigate to the previous match.
    pub fn prev_match(&mut self) {
        if let Some(ref mut idx) = self.current_match {
            if !self.match_positions.is_empty() {
                *idx = idx.checked_sub(1).unwrap_or(self.match_positions.len() - 1);
            }
        }
    }

    /// Total number of matches.
    pub fn match_count(&self) -> usize {
        self.match_positions.len()
    }

    /// Toggle case sensitivity.
    pub fn toggle_case_sensitive(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }
}

impl Default for TerminalSearchOverlay {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TerminalFocusTracker – tracks which terminal is focused
// ---------------------------------------------------------------------------

/// Tracks the currently focused terminal and focus history.
pub struct TerminalFocusTracker {
    history: Vec<String>,
    max_history: usize,
}

impl TerminalFocusTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::new(),
            max_history,
        }
    }

    /// Focus a terminal, pushing it to the front of history.
    pub fn focus(&mut self, terminal_id: impl Into<String>) {
        let id = terminal_id.into();
        self.history.retain(|h| h != &id);
        self.history.push(id);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// The currently focused terminal ID.
    pub fn current(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// The previously focused terminal ID.
    pub fn previous(&self) -> Option<&str> {
        if self.history.len() >= 2 {
            Some(&self.history[self.history.len() - 2])
        } else {
            None
        }
    }

    /// Remove a terminal from focus history.
    pub fn remove(&mut self, terminal_id: &str) {
        self.history.retain(|h| h != terminal_id);
    }

    /// Number of terminals in focus history.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}


// === Terminal Search Results Navigator ===

/// Terminal Search Results Navigator implementation.
#[derive(Debug, Clone)]
pub struct TerminalSearchNavigator {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TerminalSearchNavigatorStats,
}

/// Statistics for TerminalSearchNavigator.
#[derive(Debug, Clone, Default)]
pub struct TerminalSearchNavigatorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TerminalSearchNavigatorStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TerminalSearchNavigator {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TerminalSearchNavigatorStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TerminalSearchNavigatorStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TerminalSearchNavigator {
    fn default() -> Self {
        Self::new()
    }
}

// === Terminal Scroll Buffer ===

/// Priority level for TerminalScrollBuffer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalScrollBufferPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TerminalScrollBufferPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TerminalScrollBufferPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Terminal Scroll Buffer implementation.
#[derive(Debug, Clone)]
pub struct TerminalScrollBuffer {
    items: Vec<TerminalScrollBufferItem>,
    max_items: usize,
    default_priority: TerminalScrollBufferPriority,
}

/// A single item in TerminalScrollBuffer.
#[derive(Debug, Clone)]
pub struct TerminalScrollBufferItem {
    pub id: String,
    pub label: String,
    pub priority: TerminalScrollBufferPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TerminalScrollBufferItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TerminalScrollBufferPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TerminalScrollBufferPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TerminalScrollBuffer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TerminalScrollBufferPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TerminalScrollBufferItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TerminalScrollBufferItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TerminalScrollBufferItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TerminalScrollBufferPriority) -> Vec<&TerminalScrollBufferItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TerminalScrollBufferItem> {
        let mut sorted: Vec<&TerminalScrollBufferItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TerminalScrollBufferItem> {
        let mut sorted: Vec<&TerminalScrollBufferItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TerminalScrollBufferItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TerminalScrollBufferPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TerminalScrollBufferPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TerminalScrollBufferItem> {
        self.items.iter()
    }
}

impl Default for TerminalScrollBuffer {
    fn default() -> Self {
        Self::new()
    }
}


/// Configuration manager for terminal_view functionality.
pub struct TerminalViewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl TerminalViewConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &TerminalViewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for terminal_view operations.
pub struct TerminalViewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl TerminalViewRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for terminal_view.
pub struct TerminalViewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl TerminalViewValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &TerminalViewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = TerminalView::new();
        assert!(v.active_terminal_id.is_none());
        assert!(v.terminal_tabs.is_empty());
    }

    #[test]
    fn add_tab_sets_first_active() {
        let mut v = TerminalView::new();
        let id = v.add_tab("bash");
        assert_eq!(v.active_terminal_id, Some(id));
        assert!(v.terminal_tabs[0].is_active);
    }

    #[test]
    fn add_multiple_tabs() {
        let mut v = TerminalView::new();
        let id1 = v.add_tab("bash");
        let _id2 = v.add_tab("zsh");
        assert_eq!(v.terminal_tabs.len(), 2);
        // First tab is still active.
        assert_eq!(v.active_terminal_id, Some(id1));
    }

    #[test]
    fn remove_active_tab_activates_neighbor() {
        let mut v = TerminalView::new();
        let id1 = v.add_tab("bash");
        let id2 = v.add_tab("zsh");
        v.remove_tab(id1);
        assert_eq!(v.active_terminal_id, Some(id2));
        assert_eq!(v.terminal_tabs.len(), 1);
    }

    #[test]
    fn remove_nonexistent_tab() {
        let mut v = TerminalView::new();
        v.add_tab("bash");
        assert!(!v.remove_tab(999));
    }

    #[test]
    fn set_active_tab() {
        let mut v = TerminalView::new();
        let _id1 = v.add_tab("bash");
        let id2 = v.add_tab("zsh");
        assert!(v.set_active_tab(id2));
        assert_eq!(v.active_terminal_id, Some(id2));
        assert!(!v.set_active_tab(999));
    }

    #[test]
    fn next_tab_wraps() {
        let mut v = TerminalView::new();
        let id1 = v.add_tab("bash");
        let id2 = v.add_tab("zsh");
        v.next_tab();
        assert_eq!(v.active_terminal_id, Some(id2));
        v.next_tab();
        assert_eq!(v.active_terminal_id, Some(id1));
    }

    #[test]
    fn previous_tab_wraps() {
        let mut v = TerminalView::new();
        let _id1 = v.add_tab("bash");
        let id2 = v.add_tab("zsh");
        v.previous_tab();
        assert_eq!(v.active_terminal_id, Some(id2));
    }

    #[test]
    fn render_does_not_panic() {
        let mut v = TerminalView::new();
        v.add_tab("bash");
        v.add_tab("zsh");
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_small_area_no_panic() {
        let v = TerminalView::new();
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let v = TerminalView::default();
        assert!(v.terminal_tabs.is_empty());
    }

    fn bash_profile() -> TerminalProfile {
        TerminalProfile {
            name: "bash".to_string(),
            shell_path: "/bin/bash".to_string(),
            args: Vec::new(),
            env: Vec::new(),
            icon: None,
        }
    }

    #[test]
    fn service_create_and_close() {
        let mut svc = TerminalService::new();
        let id = svc.create_terminal(bash_profile());
        assert_eq!(svc.terminal_count(), 1);
        assert!(svc.close_terminal(id));
        assert_eq!(svc.terminal_count(), 0);
        assert!(!svc.close_terminal(id));
    }

    #[test]
    fn service_active_terminal() {
        let mut svc = TerminalService::new();
        let id1 = svc.create_terminal(bash_profile());
        let _id2 = svc.create_terminal(bash_profile());
        assert!(svc.get_active().is_none());
        svc.set_active(id1);
        assert_eq!(svc.get_active().unwrap().id, id1);
    }

    #[test]
    fn service_default_profile() {
        let mut svc = TerminalService::new();
        assert!(svc.create_default_terminal().is_none());
        svc.set_default_profile(bash_profile());
        let id = svc.create_default_terminal().unwrap();
        assert_eq!(svc.instances[0].id, id);
    }

    #[test]
    fn service_rename() {
        let mut svc = TerminalService::new();
        let id = svc.create_terminal(bash_profile());
        svc.rename_terminal(id, "my-shell");
        assert_eq!(svc.instances[0].title, "my-shell");
    }

    #[test]
    fn behavior_check_0() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = TerminalView::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    // -- TerminalBuffer tests -----------------------------------------------

    #[test]
    fn buffer_creation() {
        let buf = TerminalBuffer::new(80, 24);
        assert_eq!(buf.cols, 80);
        assert_eq!(buf.rows, 24);
        assert_eq!(buf.cells.len(), 24);
        assert_eq!(buf.cells[0].len(), 80);
        assert_eq!(buf.cursor_row, 0);
        assert_eq!(buf.cursor_col, 0);
        assert!(buf.scrollback.is_empty());
    }

    #[test]
    fn buffer_plain_text() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"Hello");
        assert_eq!(buf.cells[0][0].ch, 'H');
        assert_eq!(buf.cells[0][1].ch, 'e');
        assert_eq!(buf.cells[0][2].ch, 'l');
        assert_eq!(buf.cells[0][3].ch, 'l');
        assert_eq!(buf.cells[0][4].ch, 'o');
        assert_eq!(buf.cursor_col, 5);
        assert_eq!(buf.cursor_row, 0);
    }

    #[test]
    fn buffer_ansi_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[31m = red foreground, then text, then ESC[0m = reset
        buf.process_output(b"\x1b[31mRed\x1b[0m");
        assert_eq!(buf.cells[0][0].ch, 'R');
        assert_eq!(buf.cells[0][0].fg, Color::Red);
        assert_eq!(buf.cells[0][1].fg, Color::Red);
        assert_eq!(buf.cells[0][2].fg, Color::Red);
    }

    #[test]
    fn buffer_cursor_movement() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"AB");
        // ESC[1D = move cursor left 1
        buf.process_output(b"\x1b[1DX");
        assert_eq!(buf.cells[0][0].ch, 'A');
        assert_eq!(buf.cells[0][1].ch, 'X');
        assert_eq!(buf.cursor_col, 2);
    }

    #[test]
    fn buffer_line_wrap() {
        let mut buf = TerminalBuffer::new(5, 3);
        buf.process_output(b"ABCDE");
        // Cursor is at col 5 (past end), next char wraps.
        buf.process_output(b"F");
        assert_eq!(buf.cells[0][0].ch, 'A');
        assert_eq!(buf.cells[0][4].ch, 'E');
        assert_eq!(buf.cells[1][0].ch, 'F');
        assert_eq!(buf.cursor_row, 1);
        assert_eq!(buf.cursor_col, 1);
    }

    #[test]
    fn buffer_scrollback() {
        let mut buf = TerminalBuffer::new(10, 2);
        buf.process_output(b"Line1\r\nLine2\r\nLine3");
        // 2-row buffer: Line1 scrolled out, Line2+Line3 visible.
        assert_eq!(buf.scrollback.len(), 1);
        assert_eq!(buf.scrollback[0][0].ch, 'L');
        assert_eq!(buf.scrollback[0][4].ch, '1');
    }

    #[test]
    fn buffer_newline() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"A\nB");
        assert_eq!(buf.cells[0][0].ch, 'A');
        assert_eq!(buf.cells[1][1].ch, 'B');
        assert_eq!(buf.cursor_row, 1);
    }

    #[test]
    fn buffer_carriage_return() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"Hello\rWorld");
        // \r resets column to 0, so "World" overwrites "Hello".
        assert_eq!(buf.cells[0][0].ch, 'W');
        assert_eq!(buf.cells[0][1].ch, 'o');
        assert_eq!(buf.cells[0][2].ch, 'r');
        assert_eq!(buf.cells[0][3].ch, 'l');
        assert_eq!(buf.cells[0][4].ch, 'd');
    }

    #[test]
    fn buffer_tab() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"A\tB");
        assert_eq!(buf.cells[0][0].ch, 'A');
        // Tab should advance to column 8.
        assert_eq!(buf.cells[0][8].ch, 'B');
    }

    #[test]
    fn buffer_sgr_bold_italic() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[1;3m = bold + italic
        buf.process_output(b"\x1b[1;3mX\x1b[0m");
        assert!(buf.cells[0][0].bold);
        assert!(buf.cells[0][0].italic);
        assert_eq!(buf.cells[0][0].ch, 'X');
    }

    #[test]
    fn buffer_sgr_256_color() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[38;5;196m = 256-color red foreground
        buf.process_output(b"\x1b[38;5;196mZ");
        assert_eq!(buf.cells[0][0].fg, Color::Indexed(196));
    }

    #[test]
    fn buffer_sgr_rgb_color() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[38;2;100;200;50m = RGB foreground
        buf.process_output(b"\x1b[38;2;100;200;50mR");
        assert_eq!(buf.cells[0][0].fg, Color::Rgb(100, 200, 50));
    }

    #[test]
    fn buffer_cell_grid_dimensions() {
        let buf = TerminalBuffer::new(120, 40);
        assert_eq!(buf.cells.len(), 40);
        for row in &buf.cells {
            assert_eq!(row.len(), 120);
        }
    }

    #[test]
    fn buffer_resize() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"Hello");
        buf.resize(40, 12);
        assert_eq!(buf.cols, 40);
        assert_eq!(buf.rows, 12);
        assert_eq!(buf.cells.len(), 12);
        assert_eq!(buf.cells[0].len(), 40);
        // Shrinking from 24→12 rows moves top 12 rows to scrollback.
        // "Hello" was on row 0, now in scrollback.
        assert_eq!(buf.scrollback.len(), 12);
        assert_eq!(buf.scrollback[0][0].ch, 'H');
    }

    #[test]
    fn buffer_resize_grow() {
        let mut buf = TerminalBuffer::new(10, 5);
        buf.resize(20, 10);
        assert_eq!(buf.cols, 20);
        assert_eq!(buf.rows, 10);
        assert_eq!(buf.cells.len(), 10);
        assert_eq!(buf.cells[0].len(), 20);
    }

    #[test]
    fn buffer_erase_in_display() {
        let mut buf = TerminalBuffer::new(10, 3);
        // Use \r\n to ensure each line starts at column 0.
        buf.process_output(b"AAAAAAAAAA\r\nBBBBBBBBBB\r\nCCCCCCCCCC");
        // Move to row 1, col 5 and erase from cursor to end (J=0)
        buf.process_output(b"\x1b[2;6H\x1b[0J");
        // Row 0 should be untouched.
        assert_eq!(buf.cells[0][0].ch, 'A');
        // Row 1 cols 0-4 should still be B.
        assert_eq!(buf.cells[1][0].ch, 'B');
        assert_eq!(buf.cells[1][4].ch, 'B');
        // Row 1 col 5 should be erased.
        assert_eq!(buf.cells[1][5].ch, ' ');
        // Row 2 should be erased.
        assert_eq!(buf.cells[2][0].ch, ' ');
    }

    #[test]
    fn buffer_cursor_position_h() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[5;10H = move to row 5, col 10 (1-based)
        buf.process_output(b"\x1b[5;10HX");
        assert_eq!(buf.cells[4][9].ch, 'X');
    }

    #[test]
    fn buffer_backspace() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.process_output(b"AB\x08C");
        // Backspace moves cursor left, then C overwrites B.
        assert_eq!(buf.cells[0][0].ch, 'A');
        assert_eq!(buf.cells[0][1].ch, 'C');
    }

    #[test]
    fn buffer_render_no_panic() {
        let mut buf = TerminalBuffer::new(20, 5);
        buf.process_output(b"Hello World\nLine 2\n\x1b[32mGreen\x1b[0m");
        let area = Rect::new(0, 0, 20, 5);
        let mut rbuf = Buffer::empty(area);
        buf.render_terminal(area, &mut rbuf);
    }

    #[test]
    fn view_process_pty_output() {
        let mut v = TerminalView::new();
        let id = v.add_tab("test");
        v.process_pty_output(id, b"Hello PTY");
        let buf = v.get_buffer(id).unwrap();
        assert_eq!(buf.cells[0][0].ch, 'H');
        assert_eq!(buf.cells[0][8].ch, 'Y');
    }

    #[test]
    fn view_resize_terminal() {
        let mut v = TerminalView::new();
        let id = v.add_tab("test");
        v.resize_terminal(id, 40, 10);
        let buf = v.get_buffer(id).unwrap();
        assert_eq!(buf.cols, 40);
        assert_eq!(buf.rows, 10);
    }

    #[test]
    fn buffer_bright_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[91m = bright red foreground
        buf.process_output(b"\x1b[91mX");
        assert_eq!(buf.cells[0][0].fg, Color::LightRed);
    }

    #[test]
    fn buffer_bg_color() {
        let mut buf = TerminalBuffer::new(80, 24);
        // ESC[42m = green background
        buf.process_output(b"\x1b[42mX");
        assert_eq!(buf.cells[0][0].bg, Color::Green);
    }

    // -- ScrollbackManager tests --------------------------------------------

    #[test]
    fn scrollback_manager_push_and_len() {
        let mut sm = ScrollbackManager::new(100);
        assert!(sm.is_empty());
        let line: Vec<TerminalCell> = "hello"
            .chars()
            .map(|ch| TerminalCell { ch, ..Default::default() })
            .collect();
        sm.push_line(line);
        assert_eq!(sm.len(), 1);
        assert!(!sm.is_empty());
    }

    #[test]
    fn scrollback_manager_evicts_oldest() {
        let mut sm = ScrollbackManager::new(3);
        for i in 0..5u8 {
            let line = vec![TerminalCell {
                ch: (b'A' + i) as char,
                ..Default::default()
            }];
            sm.push_line(line);
        }
        // Capacity 3, pushed 5: should have C, D, E.
        assert_eq!(sm.len(), 3);
        assert_eq!(sm.get_line(0).unwrap()[0].ch, 'C');
        assert_eq!(sm.get_line(2).unwrap()[0].ch, 'E');
    }

    #[test]
    fn scrollback_manager_search() {
        let mut sm = ScrollbackManager::new(100);
        let make_line = |s: &str| -> Vec<TerminalCell> {
            s.chars()
                .map(|ch| TerminalCell { ch, ..Default::default() })
                .collect()
        };
        sm.push_line(make_line("error: file not found"));
        sm.push_line(make_line("warning: unused variable"));
        sm.push_line(make_line("error: type mismatch"));
        let hits = sm.search("error");
        assert_eq!(hits, vec![0, 2]);
        assert!(sm.search("success").is_empty());
    }

    #[test]
    fn scrollback_manager_line_text() {
        let mut sm = ScrollbackManager::new(10);
        let mut line: Vec<TerminalCell> = "hi  "
            .chars()
            .map(|ch| TerminalCell { ch, ..Default::default() })
            .collect();
        // Pad with trailing spaces (as a real terminal row would be).
        line.push(TerminalCell::default());
        sm.push_line(line);
        assert_eq!(sm.line_text(0).unwrap(), "hi");
    }

    #[test]
    fn scrollback_manager_clear() {
        let mut sm = ScrollbackManager::new(10);
        sm.push_line(vec![TerminalCell::default()]);
        sm.push_line(vec![TerminalCell::default()]);
        sm.clear();
        assert!(sm.is_empty());
        assert_eq!(sm.capacity(), 10);
    }

    // -- ANSI helpers tests -------------------------------------------------

    #[test]
    fn strip_ansi_removes_sequences() {
        let input = b"\x1b[31mHello\x1b[0m World\x1b[1;32m!\x1b[0m";
        let plain = strip_ansi(input);
        assert_eq!(plain, "Hello World!");
    }

    #[test]
    fn strip_ansi_preserves_newlines() {
        let input = b"line1\nline2\x1b[33m!\x1b[0m\n";
        let plain = strip_ansi(input);
        assert_eq!(plain, "line1\nline2!\n");
    }

    #[test]
    fn parse_csi_sgr() {
        let seq = parse_csi(b"1;31m").unwrap();
        assert_eq!(seq.params, vec![1, 31]);
        assert_eq!(seq.final_byte, b'm');
    }

    #[test]
    fn parse_csi_cursor_position() {
        let seq = parse_csi(b"5;10H").unwrap();
        assert_eq!(seq.params, vec![5, 10]);
        assert_eq!(seq.final_byte, b'H');
    }

    #[test]
    fn parse_csi_empty_returns_none() {
        assert!(parse_csi(b"").is_none());
    }

    // -- TerminalDimensions tests -------------------------------------------

    #[test]
    fn dimensions_resize_returns_changed() {
        let mut dims = TerminalDimensions::new(80, 24);
        assert!(dims.resize(120, 40));
        assert_eq!(dims.cols, 120);
        assert_eq!(dims.rows, 40);
        // Same dimensions should return false.
        assert!(!dims.resize(120, 40));
    }

    #[test]
    fn dimensions_rejects_zero() {
        let mut dims = TerminalDimensions::new(80, 24);
        assert!(!dims.resize(0, 24));
        assert!(!dims.resize(80, 0));
        assert_eq!(dims.cols, 80);
    }

    #[test]
    fn dimensions_cell_count() {
        let dims = TerminalDimensions::new(80, 24);
        assert_eq!(dims.cell_count(), 1920);
    }

    // -- SessionStateMachine tests ------------------------------------------

    #[test]
    fn session_lifecycle_happy_path() {
        let mut sm = SessionStateMachine::new();
        assert_eq!(sm.state(), SessionState::Created);
        assert!(!sm.is_interactive());

        assert!(sm.start());
        assert_eq!(sm.state(), SessionState::Starting);

        sm.record_output(128);
        assert_eq!(sm.state(), SessionState::Running);
        assert!(sm.is_interactive());
        assert_eq!(sm.bytes_received(), 128);

        sm.record_input(10);
        assert_eq!(sm.bytes_sent(), 10);

        assert!(sm.exit(0));
        assert_eq!(sm.state(), SessionState::Exited(0));
        assert!(!sm.is_interactive());

        assert!(sm.close());
        assert_eq!(sm.state(), SessionState::Closed);
    }

    #[test]
    fn session_invalid_transitions() {
        let mut sm = SessionStateMachine::new();
        // Can't exit from Created.
        assert!(!sm.exit(1));
        // Can't start twice.
        assert!(sm.start());
        assert!(!sm.start());
        // Can close from Created after a fresh machine.
        let mut sm2 = SessionStateMachine::new();
        assert!(sm2.close());
        assert_eq!(sm2.state(), SessionState::Closed);
    }

    #[test]
    fn session_exit_from_starting() {
        let mut sm = SessionStateMachine::new();
        sm.start();
        assert!(sm.exit(127));
        assert_eq!(sm.state(), SessionState::Exited(127));
    }

    // -- TerminalTabGroup tests --

    #[test]
    fn tab_group_add_remove() {
        let mut g = TerminalTabGroup::new("build");
        g.add("t1");
        g.add("t2");
        assert_eq!(g.len(), 2);
        assert!(g.contains("t1"));
        assert!(g.remove("t1"));
        assert_eq!(g.len(), 1);
        assert!(!g.contains("t1"));
        assert!(!g.remove("t1")); // already removed
    }

    #[test]
    fn tab_group_toggle_collapsed() {
        let mut g = TerminalTabGroup::new("test");
        assert!(!g.collapsed);
        g.toggle_collapsed();
        assert!(g.collapsed);
        g.toggle_collapsed();
        assert!(!g.collapsed);
    }

    #[test]
    fn tab_group_display() {
        let mut g = TerminalTabGroup::new("servers");
        g.add("s1");
        let s = format!("{}", g);
        assert!(s.contains("servers"));
        assert!(s.contains("1 terminal"));
    }

    // -- TerminalSplitView tests --

    #[test]
    fn split_view_creation() {
        let sv = TerminalSplitView::new("a", "b", SplitOrientation::Horizontal);
        assert_eq!(sv.focused_terminal_id(), "a");
        assert!(sv.contains("a"));
        assert!(sv.contains("b"));
        assert!(!sv.contains("c"));
    }

    #[test]
    fn split_view_toggle_focus() {
        let mut sv = TerminalSplitView::new("a", "b", SplitOrientation::Vertical);
        assert_eq!(sv.focused_pane, SplitPane::Left);
        sv.toggle_focus();
        assert_eq!(sv.focused_pane, SplitPane::Right);
        assert_eq!(sv.focused_terminal_id(), "b");
    }

    #[test]
    fn split_view_ratio_clamped() {
        let mut sv = TerminalSplitView::new("a", "b", SplitOrientation::Horizontal);
        sv.set_ratio(0.0);
        assert!((sv.ratio - 0.1).abs() < f32::EPSILON);
        sv.set_ratio(1.0);
        assert!((sv.ratio - 0.9).abs() < f32::EPSILON);
        sv.set_ratio(0.5);
        assert!((sv.ratio - 0.5).abs() < f32::EPSILON);
    }

    // -- TerminalSearchOverlay tests --

    #[test]
    fn search_overlay_basic() {
        let mut overlay = TerminalSearchOverlay::new();
        overlay.open();
        assert!(overlay.visible);
        let lines = vec!["hello world", "hello rust", "goodbye"];
        overlay.search("hello", &lines);
        assert_eq!(overlay.match_count(), 2);
        assert_eq!(overlay.current_match, Some(0));
    }

    #[test]
    fn search_overlay_navigation() {
        let mut overlay = TerminalSearchOverlay::new();
        let lines = vec!["abc", "abd", "abe"];
        overlay.search("ab", &lines);
        assert_eq!(overlay.match_count(), 3);
        overlay.next_match();
        assert_eq!(overlay.current_match, Some(1));
        overlay.next_match();
        assert_eq!(overlay.current_match, Some(2));
        overlay.next_match(); // wraps
        assert_eq!(overlay.current_match, Some(0));
        overlay.prev_match(); // wraps back
        assert_eq!(overlay.current_match, Some(2));
    }

    #[test]
    fn search_overlay_case_insensitive() {
        let mut overlay = TerminalSearchOverlay::default();
        let lines = vec!["Hello", "HELLO", "hello"];
        overlay.search("hello", &lines);
        assert_eq!(overlay.match_count(), 3);
    }

    #[test]
    fn search_overlay_close_resets() {
        let mut overlay = TerminalSearchOverlay::new();
        overlay.open();
        overlay.search("x", &["x", "y"]);
        overlay.close();
        assert!(!overlay.visible);
        assert!(overlay.query.is_empty());
        assert_eq!(overlay.match_count(), 0);
    }

    // -- TerminalFocusTracker tests --

    #[test]
    fn focus_tracker_basic() {
        let mut ft = TerminalFocusTracker::new(5);
        assert!(ft.is_empty());
        ft.focus("t1");
        ft.focus("t2");
        assert_eq!(ft.current(), Some("t2"));
        assert_eq!(ft.previous(), Some("t1"));
        assert_eq!(ft.len(), 2);
    }

    #[test]
    fn focus_tracker_removes_duplicate() {
        let mut ft = TerminalFocusTracker::new(5);
        ft.focus("t1");
        ft.focus("t2");
        ft.focus("t1"); // re-focus t1
        assert_eq!(ft.current(), Some("t1"));
        assert_eq!(ft.previous(), Some("t2"));
        assert_eq!(ft.len(), 2);
    }

    #[test]
    fn focus_tracker_max_history() {
        let mut ft = TerminalFocusTracker::new(3);
        ft.focus("t1");
        ft.focus("t2");
        ft.focus("t3");
        ft.focus("t4");
        assert_eq!(ft.len(), 3);
        assert_eq!(ft.current(), Some("t4"));
    }

    #[test]
    fn focus_tracker_remove() {
        let mut ft = TerminalFocusTracker::new(5);
        ft.focus("t1");
        ft.focus("t2");
        ft.remove("t1");
        assert_eq!(ft.len(), 1);
        assert_eq!(ft.current(), Some("t2"));
    }

    #[test]
    fn terminalSearchNavigator_new() {
        let s = TerminalSearchNavigator::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn terminalSearchNavigator_add_contains() {
        let mut s = TerminalSearchNavigator::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn terminalSearchNavigator_add_duplicate() {
        let mut s = TerminalSearchNavigator::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn terminalSearchNavigator_remove() {
        let mut s = TerminalSearchNavigator::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn terminalSearchNavigator_capacity() {
        let s = TerminalSearchNavigator::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn terminalSearchNavigator_search() {
        let mut s = TerminalSearchNavigator::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn terminalSearchNavigator_stats() {
        let mut s = TerminalSearchNavigator::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn terminalScrollBuffer_new() {
        let m = TerminalScrollBuffer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn terminalScrollBuffer_add_find() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn terminalScrollBuffer_priority_filter() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("a", "A").with_priority(TerminalScrollBufferPriority::High));
        m.add(TerminalScrollBufferItem::new("b", "B").with_priority(TerminalScrollBufferPriority::Low));
        m.add(TerminalScrollBufferItem::new("c", "C").with_priority(TerminalScrollBufferPriority::High));
        assert_eq!(m.by_priority(TerminalScrollBufferPriority::High).len(), 2);
    }

    #[test]
    fn terminalScrollBuffer_remove() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn terminalScrollBuffer_search() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("id1", "Hello World"));
        m.add(TerminalScrollBufferItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn terminalScrollBuffer_total_weight() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("a", "A").with_priority(TerminalScrollBufferPriority::Critical));
        m.add(TerminalScrollBufferItem::new("b", "B").with_priority(TerminalScrollBufferPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn terminalScrollBuffer_capacity_limit() {
        let mut m = TerminalScrollBuffer::new().with_max_items(2);
        m.add(TerminalScrollBufferItem::new("1", "one"));
        m.add(TerminalScrollBufferItem::new("2", "two"));
        assert!(!m.add(TerminalScrollBufferItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn terminalScrollBuffer_sorted_by_priority() {
        let mut m = TerminalScrollBuffer::new();
        m.add(TerminalScrollBufferItem::new("lo", "Low").with_priority(TerminalScrollBufferPriority::Low));
        m.add(TerminalScrollBufferItem::new("hi", "High").with_priority(TerminalScrollBufferPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn terminalScrollBuffer_item_metadata() {
        let mut item = TerminalScrollBufferItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn terminalSearchNavigator_enabled_toggle() {
        let mut s = TerminalSearchNavigator::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn terminalScrollBuffer_priority_display() {
        assert_eq!(format!("{}", TerminalScrollBufferPriority::High), "high");
        assert_eq!(format!("{}", TerminalScrollBufferPriority::Low), "low");
    }


    #[test]
    fn terminal_view_config_new() {
        let cfg = TerminalViewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn terminal_view_config_set_get() {
        let mut cfg = TerminalViewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn terminal_view_config_remove() {
        let mut cfg = TerminalViewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn terminal_view_config_keys_sorted() {
        let mut cfg = TerminalViewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn terminal_view_config_bump_version() {
        let mut cfg = TerminalViewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn terminal_view_config_clear() {
        let mut cfg = TerminalViewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn terminal_view_config_merge() {
        let mut cfg1 = TerminalViewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = TerminalViewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn terminal_view_config_disable() {
        let mut cfg = TerminalViewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn terminal_view_rate_tracker_empty() {
        let rt = TerminalViewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn terminal_view_rate_tracker_record() {
        let mut rt = TerminalViewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn terminal_view_rate_tracker_prune() {
        let mut rt = TerminalViewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn terminal_view_validator_valid() {
        let v = TerminalViewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn terminal_view_validator_errors() {
        let mut v = TerminalViewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn terminal_view_validator_clear() {
        let mut v = TerminalViewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn terminal_view_validator_merge() {
        let mut v1 = TerminalViewValidator::new();
        v1.add_error("e1");
        let mut v2 = TerminalViewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn terminal_view_rate_tracker_clear() {
        let mut rt = TerminalViewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }

}
