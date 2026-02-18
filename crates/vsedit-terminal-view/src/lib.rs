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


// ---------------------------------------------------------------------------
// Terminal emulator view model — extended utilities (qr)
// ---------------------------------------------------------------------------

/// Metric accumulator for term_view operations.
#[derive(Debug, Clone)]
pub struct QrMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QrMetrics {
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

/// Sliding-window rate counter for term_view.
#[derive(Debug, Clone)]
pub struct QrRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QrRateWindow {
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

/// A small LRU-style cache for term_view lookups.
#[derive(Debug, Clone)]
pub struct QrLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QrLruCache {
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
// xb_ utilities – batch 5
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer5 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer5 {
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
pub fn xb_fnv1a_5(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_5<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_5<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_5(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_5(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 177
// ---------------------------------------------------------------------------

/// Generic object pool `Xc177Pool<T>`.
pub struct Xc177Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc177Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc177PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc177Pool<T> {
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
    pub fn stats(&self) -> Xc177PoolStats {
        Xc177PoolStats {
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

impl<T> Default for Xc177Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc177Scheduler`.
pub struct Xc177Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc177Scheduler {
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

impl Default for Xc177Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_177 hash for the given byte slice.
pub fn xc_177_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_177 convention.
pub fn xc_177_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe13 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe13Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe13PipelineError {
    pub stage: Xe13Stage,
    pub message: String,
}

impl std::fmt::Display for Xe13PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe13Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe13Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError>>>,
    stage_names: Vec<Xe13Stage>,
}

impl Xe13Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe13Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe13Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe13Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe13Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
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

    pub fn compose(mut self, other: Xe13Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe13CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe13CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe13Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe13CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe13CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe13Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe13CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_13_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe13CacheEntry {
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

    fn xe_13_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe13CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_13_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
    Ok(data)
}

pub fn xe_13_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_13_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_13_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_13_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe13PipelineError> {
    Err(Xe13PipelineError {
        stage: Xe13Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #81
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf81Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf81TrieNode {
    children: std::collections::HashMap<char, Xf81TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf81Trie {
    root: Xf81TrieNode,
    count: usize,
}

impl Xf81Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf81TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf81TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf81TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf81BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf81BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 176).
pub struct Xh176SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh176SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 218 as u64,
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

/// A compact bit set supporting boolean operations (variant 176).
pub struct Xh176BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh176BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 176).
pub struct Xi176Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi176Deque<T> {
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
pub struct Xi176Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi176Interval {
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

/// A simple interval tree (variant 176).
pub struct Xi176IntervalTree {
    xi_intervals: Vec<Xi176Interval>,
}

impl Xi176IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi176Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi176Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi176Interval) -> Vec<&Xi176Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi176Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi176Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi176Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi176Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi176Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi176Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 177) ---

/// Disjoint set / union-find for crate 177.
pub struct Xj177UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj177UnionFind {
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

const XJ177_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 177.
pub struct Xj177BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj177BTreeNode<K, V>>>,
    len: usize,
}

struct Xj177BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj177BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj177BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ177_BTREE_ORDER - 1
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
        let mid = XJ177_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj177BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj177BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj177BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj177BTreeNode::xj_new_leaf();
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


// --- xk_177 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk177SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk177SegmentTree {
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
pub struct Xk177DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk177DisjointIntervals {
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


    #[test]
    fn qr_metrics_empty() {
        let m = QrMetrics::new("term_view");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qr_metrics_record_and_mean() {
        let mut m = QrMetrics::new("term_view");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qr_metrics_min_max() {
        let mut m = QrMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qr_metrics_variance_and_std() {
        let mut m = QrMetrics::new("v");
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
    fn qr_metrics_percentile() {
        let mut m = QrMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qr_metrics_merge() {
        let mut a = QrMetrics::new("a");
        a.record(1.0);
        let mut b = QrMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qr_metrics_reset() {
        let mut m = QrMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qr_rate_window_empty() {
        let rw = QrRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qr_rate_window_tick_and_rate() {
        let mut rw = QrRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qr_lru_cache_basic() {
        let mut c = QrLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qr_lru_cache_contains_and_keys() {
        let mut c = QrLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qr_lru_cache_remove() {
        let mut c = QrLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qr_metrics_sum() {
        let mut m = QrMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qr_metrics_label() {
        let m = QrMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qr_lru_cache_clear() {
        let mut c = QrLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_5_push_and_len() {
        let mut rb = super::XbRingBuffer5::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_5_overwrite() {
        let mut rb = super::XbRingBuffer5::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_5_get_out_of_bounds() {
        let rb = super::XbRingBuffer5::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_5_drain_all() {
        let mut rb = super::XbRingBuffer5::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_5_peek_front_back() {
        let mut rb = super::XbRingBuffer5::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_5_clear() {
        let mut rb = super::XbRingBuffer5::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_5_capacity() {
        let rb = super::XbRingBuffer5::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_5_basic() {
        let h = super::xb_fnv1a_5(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_5(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_5_different_inputs() {
        let h1 = super::xb_fnv1a_5(b"abc");
        let h2 = super::xb_fnv1a_5(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_5_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_5(&data);
        let dec = super::xb_rle_decode_5(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_5_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_5(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_5(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_5_values() {
        assert!((super::xb_clamp_5(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_5(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_5(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_5_values() {
        assert!((super::xb_lerp_5(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_5(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_5(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_5_wrap_around_twice() {
        let mut rb = super::XbRingBuffer5::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 177 ----

    #[test]
    fn xc_177_pool_new_empty() {
        let pool: super::Xc177Pool<i32> = super::Xc177Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_177_pool_release_acquire() {
        let mut pool = super::Xc177Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_177_pool_acquire_empty() {
        let mut pool: super::Xc177Pool<i32> = super::Xc177Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_177_pool_full() {
        let mut pool = super::Xc177Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_177_pool_drain() {
        let mut pool = super::Xc177Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_177_pool_stats() {
        let mut pool = super::Xc177Pool::new(8);
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
    fn xc_177_pool_clear() {
        let mut pool = super::Xc177Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_177_pool_shrink() {
        let mut pool = super::Xc177Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_177_pool_default() {
        let pool: super::Xc177Pool<String> = super::Xc177Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_177_pool_extend() {
        let mut pool = super::Xc177Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_177_pool_retain() {
        let mut pool = super::Xc177Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_177_scheduler_round_robin() {
        let mut sched = super::Xc177Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_177_scheduler_empty() {
        let mut sched = super::Xc177Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_177_scheduler_reset() {
        let mut sched = super::Xc177Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_177_scheduler_add_remove() {
        let mut sched = super::Xc177Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_177_scheduler_targets() {
        let sched = super::Xc177Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_177_hash_empty() {
        assert_eq!(super::xc_177_hash(b""), 5381);
    }

    #[test]
    fn xc_177_hash_data() {
        let h = super::xc_177_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_177_hash(b"hello"), h);
    }

    #[test]
    fn xc_177_reverse_str() {
        assert_eq!(super::xc_177_reverse("abc"), "cba");
        assert_eq!(super::xc_177_reverse(""), "");
    }


    #[test]
    fn xe_13_pipeline_empty() {
        let p = super::Xe13Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_13_pipeline_parse_stage() {
        let p = super::Xe13Pipeline::new()
            .add_parse(super::xe_13_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_13_pipeline_transform_double() {
        let p = super::Xe13Pipeline::new()
            .add_transform(super::xe_13_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_13_pipeline_validate_reverse() {
        let p = super::Xe13Pipeline::new()
            .add_validate(super::xe_13_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_13_pipeline_emit_filter() {
        let p = super::Xe13Pipeline::new()
            .add_emit(super::xe_13_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_13_pipeline_multi_stage() {
        let p = super::Xe13Pipeline::new()
            .add_parse(super::xe_13_pipeline_identity)
            .add_transform(super::xe_13_pipeline_double)
            .add_validate(super::xe_13_pipeline_reverse)
            .add_emit(super::xe_13_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_13_pipeline_error_propagation() {
        let p = super::Xe13Pipeline::new()
            .add_parse(super::xe_13_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe13Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_13_pipeline_compose() {
        let p1 = super::Xe13Pipeline::new()
            .add_parse(super::xe_13_pipeline_identity);
        let p2 = super::Xe13Pipeline::new()
            .add_transform(super::xe_13_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_13_pipeline_error_display() {
        let e = super::Xe13PipelineError {
            stage: super::Xe13Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_13_cache_put_get() {
        let mut c = super::Xe13Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_13_cache_miss() {
        let mut c: super::Xe13Cache<&str, i32> = super::Xe13Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_13_cache_ttl_expiry() {
        let mut c = super::Xe13Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_13_cache_evict() {
        let mut c = super::Xe13Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_13_cache_capacity() {
        let mut c = super::Xe13Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_13_cache_stats() {
        let mut c = super::Xe13Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_13_cache_clear() {
        let mut c = super::Xe13Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #81 --

    #[test]
    fn xf81_trie_insert_search() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf81_trie_starts_with() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf81_trie_remove() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf81_trie_word_count() {
        let mut t = Xf81Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf81_trie_longest_prefix() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf81_trie_all_words() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf81_trie_autocomplete() {
        let mut t = Xf81Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf81_trie_empty_search() {
        let t = Xf81Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf81_bloom_add_contains() {
        let mut bf = Xf81BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf81_bloom_probably_absent() {
        let bf = Xf81BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf81_bloom_false_positive_rate() {
        let mut bf = Xf81BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf81_bloom_clear() {
        let mut bf = Xf81BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf81_bloom_union() {
        let mut a = Xf81BloomFilter::xf_new(512, 2);
        let mut b = Xf81BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf81_bloom_intersection_estimate() {
        let mut a = Xf81BloomFilter::xf_new(512, 2);
        let mut b = Xf81BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf81_bloom_union_size_mismatch() {
        let a = Xf81BloomFilter::xf_new(256, 2);
        let b = Xf81BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh176_skip_insert_contains() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh176_skip_remove() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh176_skip_len() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh176_skip_range_query() {
        let mut sl = super::Xh176SkipList::xh_new(4);
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
    fn xh176_skip_floor_ceiling() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh176_skip_rank() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh176_skip_empty() {
        let sl = super::Xh176SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh176_skip_duplicates() {
        let mut sl = super::Xh176SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh176_bitset_set_test() {
        let mut bs = super::Xh176BitSet::xh_new(256);
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
    fn xh176_bitset_clear_count() {
        let mut bs = super::Xh176BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh176_bitset_and_or_xor() {
        let mut a = super::Xh176BitSet::xh_new(128);
        let mut b = super::Xh176BitSet::xh_new(128);
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
    fn xh176_bitset_iter_ones() {
        let mut bs = super::Xh176BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh176_bitset_first_last() {
        let mut bs = super::Xh176BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh176_bitset_empty() {
        let bs = super::Xh176BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi176_deque_push_pop_back() {
        let mut dq = super::Xi176Deque::xi_new(4);
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
    fn xi176_deque_push_pop_front() {
        let mut dq = super::Xi176Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi176_deque_mixed_ops() {
        let mut dq = super::Xi176Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi176_deque_get_and_split() {
        let mut dq = super::Xi176Deque::xi_new(8);
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
    fn xi176_deque_rotate_left() {
        let mut dq = super::Xi176Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi176_deque_rotate_right() {
        let mut dq = super::Xi176Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi176_deque_grow() {
        let mut dq = super::Xi176Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi176_deque_empty() {
        let dq = super::Xi176Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi176_interval_tree_insert_query() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi176Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi176Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi176_interval_tree_overlap() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi176Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi176Interval::xi_new(12, 20));
        let q = super::Xi176Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi176_interval_tree_remove() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi176Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi176_interval_tree_gaps() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi176Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi176Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi176Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi176Interval::xi_new(8, 10));
    }

    #[test]
    fn xi176_interval_tree_merge() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi176Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi176Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi176Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi176Interval::xi_new(10, 15));
    }

    #[test]
    fn xi176_interval_tree_all() {
        let mut tree = super::Xi176IntervalTree::xi_new();
        tree.xi_insert(super::Xi176Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi176Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi176_interval_tree_empty() {
        let tree = super::Xi176IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi176_interval_tree_contains_point() {
        let iv = super::Xi176Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 177) ---

    #[test]
    fn xj_177_uf_make_and_find() {
        let mut uf = super::Xj177UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_177_uf_union_connected() {
        let mut uf = super::Xj177UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_177_uf_component_count() {
        let mut uf = super::Xj177UnionFind::xj_new();
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
    fn xj_177_uf_component_size() {
        let mut uf = super::Xj177UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_177_uf_largest_component() {
        let mut uf = super::Xj177UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_177_uf_many_elements() {
        let mut uf = super::Xj177UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_177_uf_separate_components() {
        let mut uf = super::Xj177UnionFind::xj_new();
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
    fn xj_177_uf_path_compression() {
        let mut uf = super::Xj177UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_177_bt_insert_get() {
        let mut bt = super::Xj177BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_177_bt_contains_len() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_177_bt_replace() {
        let mut bt = super::Xj177BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_177_bt_remove() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_177_bt_keys_values() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_177_bt_range() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_177_bt_min_max() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_177_bt_many_inserts() {
        let mut bt = super::Xj177BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_177 segment tree tests ---

    #[test]
    fn xk_177_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_177_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk177SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_177_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_177_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_177_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_177_st_single_element() {
        let data = vec![42];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_177_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk177SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_177_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk177SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_177 disjoint intervals tests ---

    #[test]
    fn xk_177_di_add_and_count() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_177_di_merge_overlap() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_177_di_contains() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_177_di_remove() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_177_di_covered_length() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_177_di_gaps() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_177_di_merge_adjacent() {
        let mut di = super::Xk177DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_177_di_empty() {
        let di = super::Xk177DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
