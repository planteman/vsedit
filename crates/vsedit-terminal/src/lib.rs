//! Terminal emulation service for vsedit.
//!
//! Provides core terminal emulation including ANSI escape sequence parsing,
//! terminal buffer management, and shell integration.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use vsedit_events::{Emitter, Event};
use vsedit_lifecycle::{Disposable, DisposableStore};

// ---------------------------------------------------------------------------
// Color types
// ---------------------------------------------------------------------------

/// Terminal color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Default foreground/background.
    Default,
    /// Standard 8-color (0-7), bright (8-15), or 256-color (0-255).
    Indexed(u8),
    /// RGB true-color.
    Rgb(u8, u8, u8),
}

impl Color {
    pub const BLACK: Color = Color::Indexed(0);
    pub const RED: Color = Color::Indexed(1);
    pub const GREEN: Color = Color::Indexed(2);
    pub const YELLOW: Color = Color::Indexed(3);
    pub const BLUE: Color = Color::Indexed(4);
    pub const MAGENTA: Color = Color::Indexed(5);
    pub const CYAN: Color = Color::Indexed(6);
    pub const WHITE: Color = Color::Indexed(7);

    pub const BRIGHT_BLACK: Color = Color::Indexed(8);
    pub const BRIGHT_RED: Color = Color::Indexed(9);
    pub const BRIGHT_GREEN: Color = Color::Indexed(10);
    pub const BRIGHT_YELLOW: Color = Color::Indexed(11);
    pub const BRIGHT_BLUE: Color = Color::Indexed(12);
    pub const BRIGHT_MAGENTA: Color = Color::Indexed(13);
    pub const BRIGHT_CYAN: Color = Color::Indexed(14);
    pub const BRIGHT_WHITE: Color = Color::Indexed(15);
}

// ---------------------------------------------------------------------------
// Terminal cell
// ---------------------------------------------------------------------------

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ANSI parser
// ---------------------------------------------------------------------------

/// States for the ANSI escape sequence parser state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    CsiEntry,
    CsiParam,
    OscString,
}

/// Parsed ANSI action to apply to the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiAction {
    Print(char),
    Linefeed,
    CarriageReturn,
    Backspace,
    Tab,
    Bell,
    CursorUp(u16),
    CursorDown(u16),
    CursorForward(u16),
    CursorBack(u16),
    CursorPosition(u16, u16),
    EraseInDisplay(u16),
    EraseInLine(u16),
    ScrollUp(u16),
    ScrollDown(u16),
    Sgr(Vec<u16>),
    SetTitle(String),
}

/// State machine that converts a byte stream into `AnsiAction`s.
pub struct AnsiParser {
    state: ParserState,
    params: Vec<u16>,
    current_param: Option<u16>,
    osc_buf: String,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            current_param: None,
            osc_buf: String::new(),
        }
    }

    /// Feed a single byte into the parser, returning an optional action.
    pub fn advance(&mut self, byte: u8) -> Option<AnsiAction> {
        match self.state {
            ParserState::Ground => self.ground(byte),
            ParserState::Escape => self.escape(byte),
            ParserState::CsiEntry => self.csi_entry(byte),
            ParserState::CsiParam => self.csi_param(byte),
            ParserState::OscString => self.osc_string(byte),
        }
    }

    /// Parse an entire byte slice, collecting all actions.
    pub fn parse(&mut self, data: &[u8]) -> Vec<AnsiAction> {
        let mut actions = Vec::new();
        for &b in data {
            if let Some(action) = self.advance(b) {
                actions.push(action);
            }
        }
        actions
    }

    fn ground(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x1b => {
                self.state = ParserState::Escape;
                None
            }
            0x07 => Some(AnsiAction::Bell),
            0x08 => Some(AnsiAction::Backspace),
            0x09 => Some(AnsiAction::Tab),
            0x0a | 0x0b | 0x0c => Some(AnsiAction::Linefeed),
            0x0d => Some(AnsiAction::CarriageReturn),
            b if b >= 0x20 => Some(AnsiAction::Print(byte as char)),
            _ => None,
        }
    }

    fn escape(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'[' => {
                self.state = ParserState::CsiEntry;
                self.params.clear();
                self.current_param = None;
                None
            }
            b']' => {
                self.state = ParserState::OscString;
                self.osc_buf.clear();
                None
            }
            b'D' => {
                self.state = ParserState::Ground;
                Some(AnsiAction::ScrollUp(1))
            }
            b'M' => {
                self.state = ParserState::Ground;
                Some(AnsiAction::ScrollDown(1))
            }
            _ => {
                self.state = ParserState::Ground;
                None
            }
        }
    }

    fn csi_entry(&mut self, byte: u8) -> Option<AnsiAction> {
        self.state = ParserState::CsiParam;
        self.csi_param(byte)
    }

    fn csi_param(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'0'..=b'9' => {
                let digit = (byte - b'0') as u16;
                let cur = self.current_param.unwrap_or(0);
                self.current_param = Some(cur.saturating_mul(10).saturating_add(digit));
                None
            }
            b';' => {
                self.params.push(self.current_param.unwrap_or(0));
                self.current_param = None;
                None
            }
            b'A' => self.finish_csi(|p| AnsiAction::CursorUp(p.first().copied().unwrap_or(1).max(1))),
            b'B' => self.finish_csi(|p| AnsiAction::CursorDown(p.first().copied().unwrap_or(1).max(1))),
            b'C' => self.finish_csi(|p| AnsiAction::CursorForward(p.first().copied().unwrap_or(1).max(1))),
            b'D' => self.finish_csi(|p| AnsiAction::CursorBack(p.first().copied().unwrap_or(1).max(1))),
            b'H' | b'f' => self.finish_csi(|p| {
                let row = p.first().copied().unwrap_or(1).max(1);
                let col = p.get(1).copied().unwrap_or(1).max(1);
                AnsiAction::CursorPosition(row, col)
            }),
            b'J' => self.finish_csi(|p| AnsiAction::EraseInDisplay(p.first().copied().unwrap_or(0))),
            b'K' => self.finish_csi(|p| AnsiAction::EraseInLine(p.first().copied().unwrap_or(0))),
            b'S' => self.finish_csi(|p| AnsiAction::ScrollUp(p.first().copied().unwrap_or(1).max(1))),
            b'T' => self.finish_csi(|p| AnsiAction::ScrollDown(p.first().copied().unwrap_or(1).max(1))),
            b'm' => self.finish_csi(|p| {
                if p.is_empty() {
                    AnsiAction::Sgr(vec![0])
                } else {
                    AnsiAction::Sgr(p.to_vec())
                }
            }),
            _ => {
                self.state = ParserState::Ground;
                None
            }
        }
    }

    fn osc_string(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x07 => {
                self.state = ParserState::Ground;
                self.extract_osc_title().map(AnsiAction::SetTitle)
            }
            0x1b => {
                // ESC — accept as ST terminator.
                self.state = ParserState::Ground;
                self.extract_osc_title().map(AnsiAction::SetTitle)
            }
            _ => {
                self.osc_buf.push(byte as char);
                None
            }
        }
    }

    fn finish_csi<F>(&mut self, build: F) -> Option<AnsiAction>
    where
        F: FnOnce(&[u16]) -> AnsiAction,
    {
        self.params.push(self.current_param.unwrap_or(0));
        self.current_param = None;
        self.state = ParserState::Ground;
        Some(build(&self.params))
    }

    fn extract_osc_title(&self) -> Option<String> {
        // OSC format: `<cmd>;<text>` — accept cmd 0 and 2 for title.
        if let Some(idx) = self.osc_buf.find(';') {
            let cmd = &self.osc_buf[..idx];
            if cmd == "0" || cmd == "2" {
                return Some(self.osc_buf[idx + 1..].to_string());
            }
        }
        None
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Terminal buffer
// ---------------------------------------------------------------------------

/// Scrollback + visible terminal buffer with ANSI interpretation.
pub struct TerminalBuffer {
    cols: u16,
    rows: u16,
    lines: Vec<Vec<TerminalCell>>,
    cursor_row: u16,
    cursor_col: u16,
    scroll_offset: usize,
    max_scrollback: usize,
    parser: AnsiParser,
    current_fg: Color,
    current_bg: Color,
    current_bold: bool,
    current_italic: bool,
    current_underline: bool,
}

impl TerminalBuffer {
    pub fn new(cols: u16, rows: u16) -> Self {
        let mut lines = Vec::new();
        for _ in 0..rows {
            lines.push(Self::blank_line(cols));
        }
        Self {
            cols,
            rows,
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            max_scrollback: 10_000,
            parser: AnsiParser::new(),
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_bold: false,
            current_italic: false,
            current_underline: false,
        }
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cursor_row(&self) -> u16 {
        self.cursor_row
    }

    pub fn cursor_col(&self) -> u16 {
        self.cursor_col
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line by absolute index.
    pub fn line(&self, idx: usize) -> Option<&[TerminalCell]> {
        self.lines.get(idx).map(|v| v.as_slice())
    }

    /// Write raw bytes (UTF-8 terminal output) into the buffer.
    pub fn write_bytes(&mut self, data: &[u8]) {
        let actions = self.parser.parse(data);
        for action in actions {
            self.apply_action(action);
        }
    }

    /// Convenience: write a string.
    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    /// Clear the entire buffer and reset cursor.
    pub fn clear(&mut self) {
        self.lines.clear();
        for _ in 0..self.rows {
            self.lines.push(Self::blank_line(self.cols));
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
        self.reset_sgr();
    }

    /// Scroll the viewport up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        let max = self.max_scroll_offset();
        self.scroll_offset = (self.scroll_offset + n).min(max);
    }

    /// Scroll the viewport down by `n` lines.
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Resize the terminal grid.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        while self.lines.len() < rows as usize {
            self.lines.push(Self::blank_line(cols));
        }
        for line in &mut self.lines {
            line.resize(cols as usize, TerminalCell::default());
        }
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
    }

    /// Visible lines in the current viewport.
    pub fn visible_lines(&self) -> &[Vec<TerminalCell>] {
        let total = self.lines.len();
        let rows = self.rows as usize;
        if total <= rows {
            return &self.lines;
        }
        let bottom = total - self.scroll_offset;
        let top = bottom.saturating_sub(rows);
        &self.lines[top..bottom]
    }

    fn apply_action(&mut self, action: AnsiAction) {
        match action {
            AnsiAction::Print(ch) => self.print(ch),
            AnsiAction::Linefeed => self.linefeed(),
            AnsiAction::CarriageReturn => self.cursor_col = 0,
            AnsiAction::Backspace => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            AnsiAction::Tab => {
                let next = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next.min(self.cols - 1);
            }
            AnsiAction::Bell => {}
            AnsiAction::CursorUp(n) => {
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            AnsiAction::CursorDown(n) => {
                self.cursor_row = (self.cursor_row + n).min(self.rows - 1);
            }
            AnsiAction::CursorForward(n) => {
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }
            AnsiAction::CursorBack(n) => {
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            AnsiAction::CursorPosition(row, col) => {
                self.cursor_row = (row - 1).min(self.rows - 1);
                self.cursor_col = (col - 1).min(self.cols - 1);
            }
            AnsiAction::EraseInDisplay(mode) => self.erase_in_display(mode),
            AnsiAction::EraseInLine(mode) => self.erase_in_line(mode),
            AnsiAction::ScrollUp(n) => self.scroll_buffer_up(n),
            AnsiAction::ScrollDown(n) => self.scroll_buffer_down(n),
            AnsiAction::Sgr(params) => self.apply_sgr(&params),
            AnsiAction::SetTitle(_) => {}
        }
    }

    fn print(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.linefeed();
        }
        let abs_row = self.absolute_cursor_row();
        self.ensure_row(abs_row);
        let col = self.cursor_col as usize;
        let cell = &mut self.lines[abs_row][col];
        cell.ch = ch;
        cell.fg = self.current_fg;
        cell.bg = self.current_bg;
        cell.bold = self.current_bold;
        cell.italic = self.current_italic;
        cell.underline = self.current_underline;
        self.cursor_col += 1;
    }

    fn linefeed(&mut self) {
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        } else {
            self.lines.push(Self::blank_line(self.cols));
            self.trim_scrollback();
        }
    }

    fn erase_in_display(&mut self, mode: u16) {
        let abs_row = self.absolute_cursor_row();
        match mode {
            0 => {
                self.erase_in_line(0);
                let start = abs_row + 1;
                for r in start..self.lines.len() {
                    self.lines[r] = Self::blank_line(self.cols);
                }
            }
            1 => {
                self.erase_in_line(1);
                for r in 0..abs_row {
                    self.lines[r] = Self::blank_line(self.cols);
                }
            }
            2 | 3 => {
                for line in &mut self.lines {
                    *line = Self::blank_line(self.cols);
                }
            }
            _ => {}
        }
    }

    fn erase_in_line(&mut self, mode: u16) {
        let abs_row = self.absolute_cursor_row();
        self.ensure_row(abs_row);
        let col = self.cursor_col as usize;
        let line = &mut self.lines[abs_row];
        match mode {
            0 => {
                for c in col..line.len() {
                    line[c] = TerminalCell::default();
                }
            }
            1 => {
                for c in 0..=col.min(line.len() - 1) {
                    line[c] = TerminalCell::default();
                }
            }
            2 => {
                for cell in line.iter_mut() {
                    *cell = TerminalCell::default();
                }
            }
            _ => {}
        }
    }

    fn scroll_buffer_up(&mut self, n: u16) {
        for _ in 0..n {
            self.lines.push(Self::blank_line(self.cols));
            self.trim_scrollback();
        }
    }

    fn scroll_buffer_down(&mut self, n: u16) {
        let abs_top = self.absolute_top_row();
        for _ in 0..n {
            if self.lines.len() > self.rows as usize {
                self.lines.insert(abs_top, Self::blank_line(self.cols));
                self.lines.pop();
            }
        }
    }

    fn apply_sgr(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.reset_sgr(),
                1 => self.current_bold = true,
                3 => self.current_italic = true,
                4 => self.current_underline = true,
                22 => self.current_bold = false,
                23 => self.current_italic = false,
                24 => self.current_underline = false,
                n @ 30..=37 => self.current_fg = Color::Indexed((n - 30) as u8),
                n @ 90..=97 => self.current_fg = Color::Indexed((n - 90 + 8) as u8),
                n @ 40..=47 => self.current_bg = Color::Indexed((n - 40) as u8),
                n @ 100..=107 => self.current_bg = Color::Indexed((n - 100 + 8) as u8),
                39 => self.current_fg = Color::Default,
                49 => self.current_bg = Color::Default,
                38 => {
                    i += 1;
                    if i < params.len() {
                        match params[i] {
                            5 => {
                                i += 1;
                                if i < params.len() {
                                    self.current_fg = Color::Indexed(params[i] as u8);
                                }
                            }
                            2 => {
                                if i + 3 < params.len() {
                                    let r = params[i + 1] as u8;
                                    let g = params[i + 2] as u8;
                                    let b = params[i + 3] as u8;
                                    self.current_fg = Color::Rgb(r, g, b);
                                    i += 3;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                48 => {
                    i += 1;
                    if i < params.len() {
                        match params[i] {
                            5 => {
                                i += 1;
                                if i < params.len() {
                                    self.current_bg = Color::Indexed(params[i] as u8);
                                }
                            }
                            2 => {
                                if i + 3 < params.len() {
                                    let r = params[i + 1] as u8;
                                    let g = params[i + 2] as u8;
                                    let b = params[i + 3] as u8;
                                    self.current_bg = Color::Rgb(r, g, b);
                                    i += 3;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn reset_sgr(&mut self) {
        self.current_fg = Color::Default;
        self.current_bg = Color::Default;
        self.current_bold = false;
        self.current_italic = false;
        self.current_underline = false;
    }

    fn blank_line(cols: u16) -> Vec<TerminalCell> {
        vec![TerminalCell::default(); cols as usize]
    }

    fn absolute_cursor_row(&self) -> usize {
        let total = self.lines.len();
        let rows = self.rows as usize;
        if total <= rows {
            self.cursor_row as usize
        } else {
            (total - rows) + self.cursor_row as usize
        }
    }

    fn absolute_top_row(&self) -> usize {
        let total = self.lines.len();
        let rows = self.rows as usize;
        total.saturating_sub(rows)
    }

    fn ensure_row(&mut self, idx: usize) {
        while self.lines.len() <= idx {
            self.lines.push(Self::blank_line(self.cols));
        }
    }

    fn max_scroll_offset(&self) -> usize {
        self.lines.len().saturating_sub(self.rows as usize)
    }

    fn trim_scrollback(&mut self) {
        let max_lines = self.max_scrollback + self.rows as usize;
        while self.lines.len() > max_lines {
            self.lines.remove(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Shell configuration
// ---------------------------------------------------------------------------

/// Known shell profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellProfile {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Sh,
    Other,
}

impl ShellProfile {
    /// Detect profile from shell path.
    pub fn from_path(path: &str) -> Self {
        if let Some(name) = path.rsplit('/').next() {
            match name {
                "bash" => Self::Bash,
                "zsh" => Self::Zsh,
                "fish" => Self::Fish,
                "pwsh" | "powershell" => Self::PowerShell,
                "sh" | "dash" => Self::Sh,
                _ => Self::Other,
            }
        } else {
            Self::Other
        }
    }

    /// Default arguments for this shell profile.
    pub fn default_args(&self) -> Vec<String> {
        match self {
            Self::Bash => vec!["--login".into()],
            Self::Zsh => vec!["--login".into()],
            Self::Fish => vec!["--login".into()],
            Self::PowerShell => vec!["-NoLogo".into()],
            Self::Sh | Self::Other => Vec::new(),
        }
    }
}

/// Shell configuration for spawning terminals.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub profile: ShellProfile,
}

impl ShellConfig {
    pub fn new(path: PathBuf) -> Self {
        let profile = ShellProfile::from_path(&path.to_string_lossy());
        let args = profile.default_args();
        Self {
            path,
            args,
            env: HashMap::new(),
            profile,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Detect the default shell for the current platform.
pub fn detect_default_shell() -> PathBuf {
    if let Ok(shell) = env::var("SHELL") {
        if !shell.is_empty() {
            return PathBuf::from(shell);
        }
    }
    if cfg!(target_os = "windows") {
        return PathBuf::from("pwsh.exe");
    }
    PathBuf::from("/bin/sh")
}

/// Build a `ShellConfig` from the detected default shell.
pub fn default_shell_config() -> ShellConfig {
    ShellConfig::new(detect_default_shell())
}

// ---------------------------------------------------------------------------
// Terminal instance
// ---------------------------------------------------------------------------

/// Unique identifier for a terminal instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(u64);

impl TerminalId {
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TerminalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal-{}", self.0)
    }
}

/// Running state of a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalState {
    Running,
    Stopped,
}

/// Events emitted by a terminal instance.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    DataWritten { id: TerminalId, len: usize },
    TitleChanged { id: TerminalId, title: String },
    Resized { id: TerminalId, cols: u16, rows: u16 },
    Closed { id: TerminalId },
}

/// Represents a single terminal session.
pub struct TerminalInstance {
    id: TerminalId,
    title: String,
    cwd: PathBuf,
    shell_config: ShellConfig,
    state: TerminalState,
    buffer: TerminalBuffer,
    event_emitter: Emitter<TerminalEvent>,
    disposables: DisposableStore,
}

impl TerminalInstance {
    pub fn new(
        id: TerminalId,
        title: impl Into<String>,
        cwd: PathBuf,
        shell_config: ShellConfig,
        cols: u16,
        rows: u16,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            cwd,
            shell_config,
            state: TerminalState::Running,
            buffer: TerminalBuffer::new(cols, rows),
            event_emitter: Emitter::new(),
            disposables: DisposableStore::new(),
        }
    }

    pub fn id(&self) -> TerminalId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.event_emitter.fire(&TerminalEvent::TitleChanged {
            id: self.id,
            title: self.title.clone(),
        });
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    pub fn shell_config(&self) -> &ShellConfig {
        &self.shell_config
    }

    pub fn state(&self) -> TerminalState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        self.state == TerminalState::Running
    }

    pub fn buffer(&self) -> &TerminalBuffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut TerminalBuffer {
        &mut self.buffer
    }

    /// Write data into the terminal buffer (simulates PTY output).
    pub fn write_output(&mut self, data: &[u8]) {
        self.buffer.write_bytes(data);
        self.event_emitter.fire(&TerminalEvent::DataWritten {
            id: self.id,
            len: data.len(),
        });
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.buffer.resize(cols, rows);
        self.event_emitter.fire(&TerminalEvent::Resized {
            id: self.id,
            cols,
            rows,
        });
    }

    /// Mark the terminal as stopped.
    pub fn close(&mut self) {
        self.state = TerminalState::Stopped;
        self.event_emitter
            .fire(&TerminalEvent::Closed { id: self.id });
    }

    /// Get the event stream.
    pub fn on_event(&self) -> Event<TerminalEvent> {
        self.event_emitter.event()
    }
}

impl Disposable for TerminalInstance {
    fn dispose(&self) {
        self.disposables.dispose();
    }

    fn is_disposed(&self) -> bool {
        self.disposables.is_disposed()
    }
}

// ---------------------------------------------------------------------------
// Terminal service
// ---------------------------------------------------------------------------

/// Events emitted by the service itself.
#[derive(Debug, Clone)]
pub enum TerminalServiceEvent {
    InstanceCreated(TerminalId),
    InstanceDestroyed(TerminalId),
    ActiveChanged(Option<TerminalId>),
}

/// Manages multiple terminal instances.
pub struct TerminalService {
    instances: HashMap<TerminalId, TerminalInstance>,
    active_id: Option<TerminalId>,
    next_id: u64,
    event_emitter: Emitter<TerminalServiceEvent>,
    disposables: DisposableStore,
}

impl TerminalService {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            active_id: None,
            next_id: 1,
            event_emitter: Emitter::new(),
            disposables: DisposableStore::new(),
        }
    }

    /// Create a new terminal instance and return its id.
    pub fn create(
        &mut self,
        title: impl Into<String>,
        cwd: PathBuf,
        shell_config: ShellConfig,
        cols: u16,
        rows: u16,
    ) -> TerminalId {
        let id = TerminalId(self.next_id);
        self.next_id += 1;
        let instance = TerminalInstance::new(id, title, cwd, shell_config, cols, rows);
        self.instances.insert(id, instance);
        if self.active_id.is_none() {
            self.active_id = Some(id);
            self.event_emitter
                .fire(&TerminalServiceEvent::ActiveChanged(Some(id)));
        }
        self.event_emitter
            .fire(&TerminalServiceEvent::InstanceCreated(id));
        id
    }

    /// Destroy a terminal instance.
    pub fn destroy(&mut self, id: TerminalId) -> bool {
        if let Some(instance) = self.instances.remove(&id) {
            instance.dispose();
            self.event_emitter
                .fire(&TerminalServiceEvent::InstanceDestroyed(id));
            if self.active_id == Some(id) {
                self.active_id = self.instances.keys().next().copied();
                self.event_emitter
                    .fire(&TerminalServiceEvent::ActiveChanged(self.active_id));
            }
            true
        } else {
            false
        }
    }

    /// Get a reference to a terminal instance.
    pub fn get(&self, id: TerminalId) -> Option<&TerminalInstance> {
        self.instances.get(&id)
    }

    /// Get a mutable reference to a terminal instance.
    pub fn get_mut(&mut self, id: TerminalId) -> Option<&mut TerminalInstance> {
        self.instances.get_mut(&id)
    }

    /// List all terminal ids.
    pub fn list(&self) -> Vec<TerminalId> {
        self.instances.keys().copied().collect()
    }

    /// Number of active terminals.
    pub fn count(&self) -> usize {
        self.instances.len()
    }

    /// Get the active terminal id.
    pub fn get_active(&self) -> Option<TerminalId> {
        self.active_id
    }

    /// Get a reference to the active terminal instance.
    pub fn get_active_instance(&self) -> Option<&TerminalInstance> {
        self.active_id.and_then(|id| self.instances.get(&id))
    }

    /// Get a mutable reference to the active terminal instance.
    pub fn get_active_instance_mut(&mut self) -> Option<&mut TerminalInstance> {
        self.active_id.and_then(|id| self.instances.get_mut(&id))
    }

    /// Set the active terminal.
    pub fn set_active(&mut self, id: TerminalId) -> bool {
        if self.instances.contains_key(&id) {
            self.active_id = Some(id);
            self.event_emitter
                .fire(&TerminalServiceEvent::ActiveChanged(Some(id)));
            true
        } else {
            false
        }
    }

    /// Get the event stream for service-level events.
    pub fn on_event(&self) -> Event<TerminalServiceEvent> {
        self.event_emitter.event()
    }
}

impl Default for TerminalService {
    fn default() -> Self {
        Self::new()
    }
}

impl Disposable for TerminalService {
    fn dispose(&self) {
        for instance in self.instances.values() {
            instance.dispose();
        }
        self.disposables.dispose();
    }

    fn is_disposed(&self) -> bool {
        self.disposables.is_disposed()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- AnsiParser tests ---------------------------------------------------

    #[test]
    fn parse_printable_chars() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"Hello");
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0], AnsiAction::Print('H'));
        assert_eq!(actions[4], AnsiAction::Print('o'));
    }

    #[test]
    fn parse_c0_controls() {
        let mut parser = AnsiParser::new();
        assert_eq!(parser.advance(0x0a), Some(AnsiAction::Linefeed));
        assert_eq!(parser.advance(0x0d), Some(AnsiAction::CarriageReturn));
        assert_eq!(parser.advance(0x08), Some(AnsiAction::Backspace));
        assert_eq!(parser.advance(0x09), Some(AnsiAction::Tab));
        assert_eq!(parser.advance(0x07), Some(AnsiAction::Bell));
    }

    #[test]
    fn parse_sgr_reset() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[0m");
        assert_eq!(actions, vec![AnsiAction::Sgr(vec![0])]);
    }

    #[test]
    fn parse_sgr_no_params() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[m");
        assert_eq!(actions, vec![AnsiAction::Sgr(vec![0])]);
    }

    #[test]
    fn parse_sgr_bold_fg_color() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[1;31m");
        assert_eq!(actions, vec![AnsiAction::Sgr(vec![1, 31])]);
    }

    #[test]
    fn parse_sgr_256_color() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[38;5;196m");
        assert_eq!(actions, vec![AnsiAction::Sgr(vec![38, 5, 196])]);
    }

    #[test]
    fn parse_sgr_rgb_color() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[38;2;255;128;0m");
        assert_eq!(actions, vec![AnsiAction::Sgr(vec![38, 2, 255, 128, 0])]);
    }

    #[test]
    fn parse_cursor_up() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[3A");
        assert_eq!(actions, vec![AnsiAction::CursorUp(3)]);
    }

    #[test]
    fn parse_cursor_down_default() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[B");
        assert_eq!(actions, vec![AnsiAction::CursorDown(1)]);
    }

    #[test]
    fn parse_cursor_forward_back() {
        let mut parser = AnsiParser::new();
        let fwd = parser.parse(b"\x1b[5C");
        assert_eq!(fwd, vec![AnsiAction::CursorForward(5)]);
        let back = parser.parse(b"\x1b[2D");
        assert_eq!(back, vec![AnsiAction::CursorBack(2)]);
    }

    #[test]
    fn parse_cursor_position() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[10;20H");
        assert_eq!(actions, vec![AnsiAction::CursorPosition(10, 20)]);
    }

    #[test]
    fn parse_erase_in_display() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[2J");
        assert_eq!(actions, vec![AnsiAction::EraseInDisplay(2)]);
    }

    #[test]
    fn parse_erase_in_line() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b[K");
        assert_eq!(actions, vec![AnsiAction::EraseInLine(0)]);
    }

    #[test]
    fn parse_scroll_up_down() {
        let mut parser = AnsiParser::new();
        let up = parser.parse(b"\x1b[3S");
        assert_eq!(up, vec![AnsiAction::ScrollUp(3)]);
        let down = parser.parse(b"\x1b[2T");
        assert_eq!(down, vec![AnsiAction::ScrollDown(2)]);
    }

    #[test]
    fn parse_osc_title() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b]0;My Title\x07");
        assert_eq!(actions, vec![AnsiAction::SetTitle("My Title".into())]);
    }

    #[test]
    fn parse_osc_title_st_terminator() {
        let mut parser = AnsiParser::new();
        let actions = parser.parse(b"\x1b]2;Another Title\x1b\\");
        // ESC terminates the OSC, backslash is consumed as ground char.
        assert!(actions.len() >= 1);
        assert_eq!(actions[0], AnsiAction::SetTitle("Another Title".into()));
    }

    #[test]
    fn parse_escape_scroll_shortcuts() {
        let mut parser = AnsiParser::new();
        let down = parser.parse(b"\x1bD");
        assert_eq!(down, vec![AnsiAction::ScrollUp(1)]);
        let up = parser.parse(b"\x1bM");
        assert_eq!(up, vec![AnsiAction::ScrollDown(1)]);
    }

    // -- TerminalBuffer tests -----------------------------------------------

    #[test]
    fn buffer_write_str() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("ABC");
        assert_eq!(buf.cursor_col(), 3);
        assert_eq!(buf.cursor_row(), 0);
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, 'A');
        assert_eq!(line[1].ch, 'B');
        assert_eq!(line[2].ch, 'C');
    }

    #[test]
    fn buffer_linefeed_wraps() {
        let mut buf = TerminalBuffer::new(80, 3);
        buf.write_str("line1\r\nline2\r\nline3\r\nline4");
        assert!(buf.line_count() > 3);
    }

    #[test]
    fn buffer_clear() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello, World!");
        buf.clear();
        assert_eq!(buf.cursor_col(), 0);
        assert_eq!(buf.cursor_row(), 0);
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, ' ');
    }

    #[test]
    fn buffer_scroll_viewport() {
        let mut buf = TerminalBuffer::new(80, 3);
        for i in 0..10 {
            buf.write_str(&format!("line{}\r\n", i));
        }
        buf.scroll_up(2);
        assert_eq!(buf.scroll_offset(), 2);
        buf.scroll_down(1);
        assert_eq!(buf.scroll_offset(), 1);
        buf.scroll_down(100);
        assert_eq!(buf.scroll_offset(), 0);
    }

    #[test]
    fn buffer_resize() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello");
        buf.resize(40, 12);
        assert_eq!(buf.cols(), 40);
        assert_eq!(buf.rows(), 12);
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, 'H');
    }

    #[test]
    fn buffer_resize_zero_noop() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.resize(0, 0);
        assert_eq!(buf.cols(), 80);
        assert_eq!(buf.rows(), 24);
    }

    #[test]
    fn buffer_sgr_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[1;31mX\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, 'X');
        assert!(line[0].bold);
        assert_eq!(line[0].fg, Color::Indexed(1));
    }

    #[test]
    fn buffer_sgr_256_color() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[38;5;42mA\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].fg, Color::Indexed(42));
    }

    #[test]
    fn buffer_sgr_rgb_color() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[38;2;100;200;50mR\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].fg, Color::Rgb(100, 200, 50));
    }

    #[test]
    fn buffer_sgr_background_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[44mB\x1b[48;5;200mC\x1b[48;2;10;20;30mD\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].bg, Color::Indexed(4));
        assert_eq!(line[1].bg, Color::Indexed(200));
        assert_eq!(line[2].bg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn buffer_cursor_movement() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[5;10H");
        assert_eq!(buf.cursor_row(), 4);
        assert_eq!(buf.cursor_col(), 9);
    }

    #[test]
    fn buffer_erase_in_line() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("ABCDEFGH");
        // Move cursor to column 4 (1-based) via CursorPosition
        buf.write_bytes(b"\x1b[1;4H");
        buf.write_bytes(b"\x1b[K");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, 'A');
        assert_eq!(line[1].ch, 'B');
        assert_eq!(line[2].ch, 'C');
        assert_eq!(line[3].ch, ' ');
    }

    #[test]
    fn buffer_tab_stops() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"A\tB");
        assert_eq!(buf.line(0).unwrap()[0].ch, 'A');
        assert_eq!(buf.line(0).unwrap()[8].ch, 'B');
    }

    #[test]
    fn buffer_backspace() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("AB");
        buf.write_bytes(b"\x08C");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].ch, 'A');
        assert_eq!(line[1].ch, 'C');
    }

    #[test]
    fn buffer_bright_foreground_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[90mX\x1b[97mY\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].fg, Color::Indexed(8));
        assert_eq!(line[1].fg, Color::Indexed(15));
    }

    #[test]
    fn buffer_bright_background_colors() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_bytes(b"\x1b[100mX\x1b[107mY\x1b[0m");
        let line = buf.line(0).unwrap();
        assert_eq!(line[0].bg, Color::Indexed(8));
        assert_eq!(line[1].bg, Color::Indexed(15));
    }

    // -- TerminalService tests ----------------------------------------------

    #[test]
    fn service_create_terminal() {
        let mut svc = TerminalService::new();
        let shell = default_shell_config();
        let id = svc.create("Test", PathBuf::from("/tmp"), shell, 80, 24);
        assert_eq!(svc.count(), 1);
        assert!(svc.get(id).is_some());
        assert_eq!(svc.get(id).unwrap().title(), "Test");
    }

    #[test]
    fn service_auto_activates_first() {
        let mut svc = TerminalService::new();
        let shell = default_shell_config();
        let id = svc.create("First", PathBuf::from("/tmp"), shell, 80, 24);
        assert_eq!(svc.get_active(), Some(id));
    }

    #[test]
    fn service_destroy_terminal() {
        let mut svc = TerminalService::new();
        let shell = default_shell_config();
        let id = svc.create("Test", PathBuf::from("/tmp"), shell, 80, 24);
        assert!(svc.destroy(id));
        assert_eq!(svc.count(), 0);
        assert!(svc.get(id).is_none());
    }

    #[test]
    fn service_destroy_nonexistent() {
        let mut svc = TerminalService::new();
        assert!(!svc.destroy(TerminalId(999)));
    }

    #[test]
    fn service_list_terminals() {
        let mut svc = TerminalService::new();
        let shell1 = default_shell_config();
        let shell2 = default_shell_config();
        let id1 = svc.create("T1", PathBuf::from("/tmp"), shell1, 80, 24);
        let id2 = svc.create("T2", PathBuf::from("/tmp"), shell2, 80, 24);
        let list = svc.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&id1));
        assert!(list.contains(&id2));
    }

    #[test]
    fn service_set_active() {
        let mut svc = TerminalService::new();
        let shell1 = default_shell_config();
        let shell2 = default_shell_config();
        let _id1 = svc.create("T1", PathBuf::from("/tmp"), shell1, 80, 24);
        let id2 = svc.create("T2", PathBuf::from("/tmp"), shell2, 80, 24);
        assert!(svc.set_active(id2));
        assert_eq!(svc.get_active(), Some(id2));
    }

    #[test]
    fn service_set_active_nonexistent() {
        let mut svc = TerminalService::new();
        assert!(!svc.set_active(TerminalId(999)));
    }

    #[test]
    fn service_active_changes_on_destroy() {
        let mut svc = TerminalService::new();
        let shell1 = default_shell_config();
        let shell2 = default_shell_config();
        let id1 = svc.create("T1", PathBuf::from("/tmp"), shell1, 80, 24);
        let _id2 = svc.create("T2", PathBuf::from("/tmp"), shell2, 80, 24);
        svc.set_active(id1);
        svc.destroy(id1);
        assert!(svc.get_active().is_some());
    }

    #[test]
    fn service_get_active_instance() {
        let mut svc = TerminalService::new();
        let shell = default_shell_config();
        let id = svc.create("Test", PathBuf::from("/tmp"), shell, 80, 24);
        let inst = svc.get_active_instance().unwrap();
        assert_eq!(inst.id(), id);
        assert_eq!(inst.title(), "Test");
    }

    // -- Shell detection tests ----------------------------------------------

    #[test]
    fn detect_shell_returns_path() {
        let path = detect_default_shell();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn shell_profile_from_path() {
        assert_eq!(ShellProfile::from_path("/bin/bash"), ShellProfile::Bash);
        assert_eq!(ShellProfile::from_path("/usr/bin/zsh"), ShellProfile::Zsh);
        assert_eq!(ShellProfile::from_path("/usr/bin/fish"), ShellProfile::Fish);
        assert_eq!(
            ShellProfile::from_path("/usr/bin/pwsh"),
            ShellProfile::PowerShell
        );
        assert_eq!(ShellProfile::from_path("/bin/sh"), ShellProfile::Sh);
        assert_eq!(
            ShellProfile::from_path("/bin/unknown"),
            ShellProfile::Other
        );
    }

    #[test]
    fn shell_config_default_args() {
        let cfg = ShellConfig::new(PathBuf::from("/bin/bash"));
        assert_eq!(cfg.profile, ShellProfile::Bash);
        assert_eq!(cfg.args, vec!["--login".to_string()]);
    }

    #[test]
    fn shell_config_with_env() {
        let cfg =
            ShellConfig::new(PathBuf::from("/bin/bash")).with_env("TERM", "xterm-256color");
        assert_eq!(cfg.env.get("TERM").unwrap(), "xterm-256color");
    }

    // -- Color tests --------------------------------------------------------

    #[test]
    fn color_standard_constants() {
        assert_eq!(Color::RED, Color::Indexed(1));
        assert_eq!(Color::BRIGHT_CYAN, Color::Indexed(14));
    }

    #[test]
    fn color_rgb_equality() {
        assert_eq!(Color::Rgb(255, 0, 0), Color::Rgb(255, 0, 0));
        assert_ne!(Color::Rgb(255, 0, 0), Color::Rgb(0, 255, 0));
    }

    // -- TerminalInstance tests ---------------------------------------------

    #[test]
    fn instance_write_output() {
        let shell = default_shell_config();
        let mut inst = TerminalInstance::new(
            TerminalId(1),
            "Test",
            PathBuf::from("/tmp"),
            shell,
            80,
            24,
        );
        inst.write_output(b"Hello");
        let line = inst.buffer().line(0).unwrap();
        assert_eq!(line[0].ch, 'H');
    }

    #[test]
    fn instance_close() {
        let shell = default_shell_config();
        let mut inst = TerminalInstance::new(
            TerminalId(1),
            "Test",
            PathBuf::from("/tmp"),
            shell,
            80,
            24,
        );
        assert!(inst.is_running());
        inst.close();
        assert!(!inst.is_running());
        assert_eq!(inst.state(), TerminalState::Stopped);
    }

    #[test]
    fn instance_resize() {
        let shell = default_shell_config();
        let mut inst = TerminalInstance::new(
            TerminalId(1),
            "Test",
            PathBuf::from("/tmp"),
            shell,
            80,
            24,
        );
        inst.resize(120, 40);
        assert_eq!(inst.buffer().cols(), 120);
        assert_eq!(inst.buffer().rows(), 40);
    }

    #[test]
    fn terminal_id_display() {
        let id = TerminalId(42);
        assert_eq!(format!("{}", id), "terminal-42");
    }
}
