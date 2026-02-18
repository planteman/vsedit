//! Terminal emulation service for vsedit.
//!
//! Provides core terminal emulation including ANSI escape sequence parsing,
//! terminal buffer management, and shell integration.

use std::fmt;
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

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
// PTY configuration and session
// ---------------------------------------------------------------------------

/// Configuration for spawning a PTY session.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            shell: detect_default_shell().to_string_lossy().into_owned(),
            cols: 80,
            rows: 24,
            cwd: None,
            env: HashMap::new(),
        }
    }
}

/// A pseudo-terminal session wrapping a shell process with piped I/O.
pub struct PtySession {
    child: Child,
    buffer: TerminalBuffer,
}

impl PtySession {
    /// Spawn a new shell process.
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self, std::io::Error> {
        let child = Command::new(shell)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("TERM", "xterm-256color")
            .env("COLUMNS", cols.to_string())
            .env("LINES", rows.to_string())
            .spawn()?;

        Ok(Self {
            child,
            buffer: TerminalBuffer::new(cols, rows),
        })
    }

    /// Spawn from a `PtyConfig`.
    pub fn spawn_with_config(config: &PtyConfig) -> Result<Self, std::io::Error> {
        let mut cmd = Command::new(&config.shell);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("TERM", "xterm-256color")
            .env("COLUMNS", config.cols.to_string())
            .env("LINES", config.rows.to_string());

        if let Some(ref cwd) = config.cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let child = cmd.spawn()?;
        Ok(Self {
            child,
            buffer: TerminalBuffer::new(config.cols, config.rows),
        })
    }

    /// Write input to the shell (user keystrokes).
    pub fn write_input(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        if let Some(ref mut stdin) = self.child.stdin {
            stdin.write_all(data)?;
            stdin.flush()?;
        }
        Ok(())
    }

    /// Read available output from the shell and feed into terminal buffer.
    pub fn read_output(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![0u8; 4096];
        if let Some(ref mut stdout) = self.child.stdout {
            match stdout.read(&mut buf) {
                Ok(n) if n > 0 => {
                    buf.truncate(n);
                    let text = String::from_utf8_lossy(&buf);
                    self.buffer.write_str(&text);
                    Ok(buf)
                }
                Ok(_) => Ok(Vec::new()),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
                Err(e) => Err(e),
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        self.child.kill()
    }

    /// Get the terminal buffer for rendering.
    pub fn buffer(&self) -> &TerminalBuffer {
        &self.buffer
    }

    /// Get mutable buffer reference.
    pub fn buffer_mut(&mut self) -> &mut TerminalBuffer {
        &mut self.buffer
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.kill();
    }
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

    /// Create a terminal instance backed by a real shell process.
    pub fn create_with_pty(
        &mut self,
        shell: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalId, std::io::Error> {
        let shell_path = shell.unwrap_or_else(|| {
            if cfg!(windows) {
                "cmd.exe"
            } else {
                "/bin/sh"
            }
        });
        let _pty = PtySession::spawn(shell_path, cols, rows)?;
        let cwd = std::env::current_dir().unwrap_or_default();
        let shell_config = ShellConfig::new(PathBuf::from(shell_path));
        let id = self.create("Terminal (PTY)", cwd, shell_config, cols, rows);
        Ok(id)
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
// Terminal link detection
// ---------------------------------------------------------------------------

/// A detected link in terminal output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLink {
    /// Row in the buffer (absolute index).
    pub row: usize,
    /// Start column (inclusive).
    pub col_start: usize,
    /// End column (exclusive).
    pub col_end: usize,
    /// The URL or file path text.
    pub target: String,
    /// The kind of link detected.
    pub kind: LinkKind,
}

/// Classification of a detected link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// An http:// or https:// URL.
    Url,
    /// A file path, possibly with line/column (e.g. `src/main.rs:42:5`).
    FilePath,
}

/// Scans a single line of terminal cells for links.
pub fn detect_links_in_line(row: usize, cells: &[TerminalCell]) -> Vec<TerminalLink> {
    let text: String = cells.iter().map(|c| c.ch).collect();
    let mut links = Vec::new();

    // Detect URLs (http:// and https://)
    let mut search_from = 0;
    while search_from < text.len() {
        let haystack = &text[search_from..];
        let url_start = haystack
            .find("https://")
            .or_else(|| haystack.find("http://"));
        if let Some(rel) = url_start {
            let abs_start = search_from + rel;
            let end = text[abs_start..]
                .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '>' || c == '"' || c == '\'')
                .map(|e| abs_start + e)
                .unwrap_or(text.len());
            // Trim trailing punctuation that's unlikely part of the URL.
            let mut trimmed_end = end;
            while trimmed_end > abs_start
                && matches!(text.as_bytes()[trimmed_end - 1], b'.' | b',' | b';' | b':')
            {
                trimmed_end -= 1;
            }
            if trimmed_end > abs_start + 8 {
                links.push(TerminalLink {
                    row,
                    col_start: abs_start,
                    col_end: trimmed_end,
                    target: text[abs_start..trimmed_end].to_string(),
                    kind: LinkKind::Url,
                });
            }
            search_from = trimmed_end;
        } else {
            break;
        }
    }

    // Detect file paths with line numbers (e.g. `src/main.rs:42` or `./foo/bar.txt:10:5`)
    search_from = 0;
    while search_from < text.len() {
        let haystack = &text[search_from..];
        // Look for patterns like `word/word.ext:digits`
        if let Some(colon_pos) = haystack.find(':') {
            let abs_colon = search_from + colon_pos;
            // Check if the character after the colon is a digit
            if abs_colon + 1 < text.len() && text.as_bytes()[abs_colon + 1].is_ascii_digit() {
                // Walk backwards to find path start
                let mut path_start = abs_colon;
                while path_start > search_from {
                    let prev = text.as_bytes()[path_start - 1];
                    if prev.is_ascii_alphanumeric()
                        || prev == b'/'
                        || prev == b'.'
                        || prev == b'-'
                        || prev == b'_'
                    {
                        path_start -= 1;
                    } else {
                        break;
                    }
                }
                // Must contain a dot or slash to look like a file path
                let path_part = &text[path_start..abs_colon];
                if (path_part.contains('.') || path_part.contains('/')) && !path_part.is_empty() {
                    // Walk forward to capture `:line` and optional `:col`
                    let mut path_end = abs_colon + 1;
                    while path_end < text.len() && text.as_bytes()[path_end].is_ascii_digit() {
                        path_end += 1;
                    }
                    if path_end < text.len() && text.as_bytes()[path_end] == b':' {
                        let maybe_col = path_end + 1;
                        if maybe_col < text.len() && text.as_bytes()[maybe_col].is_ascii_digit() {
                            path_end = maybe_col;
                            while path_end < text.len()
                                && text.as_bytes()[path_end].is_ascii_digit()
                            {
                                path_end += 1;
                            }
                        }
                    }
                    // Don't add if this range overlaps a URL we already found
                    let overlaps = links.iter().any(|l| {
                        l.row == row && path_start < l.col_end && path_end > l.col_start
                    });
                    if !overlaps {
                        links.push(TerminalLink {
                            row,
                            col_start: path_start,
                            col_end: path_end,
                            target: text[path_start..path_end].to_string(),
                            kind: LinkKind::FilePath,
                        });
                    }
                }
            }
            search_from = abs_colon + 1;
        } else {
            break;
        }
    }

    links
}

/// Scan all lines of a terminal buffer for links.
pub fn detect_links(buffer: &TerminalBuffer) -> Vec<TerminalLink> {
    let mut all = Vec::new();
    for row in 0..buffer.line_count() {
        if let Some(cells) = buffer.line(row) {
            all.extend(detect_links_in_line(row, cells));
        }
    }
    all
}

// ---------------------------------------------------------------------------
// Terminal search
// ---------------------------------------------------------------------------

/// A match found by terminal search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Row in the buffer (absolute index).
    pub row: usize,
    /// Start column (inclusive).
    pub col_start: usize,
    /// End column (exclusive).
    pub col_end: usize,
}

/// Options controlling terminal search behavior.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex: false,
        }
    }
}

/// Search the terminal buffer for a needle string.
pub fn search_buffer(
    buffer: &TerminalBuffer,
    needle: &str,
    options: &SearchOptions,
) -> Vec<SearchMatch> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let needle_cmp: String = if options.case_sensitive {
        needle.to_string()
    } else {
        needle.to_lowercase()
    };

    for row in 0..buffer.line_count() {
        if let Some(cells) = buffer.line(row) {
            let line_text: String = cells.iter().map(|c| c.ch).collect();
            let line_cmp: String = if options.case_sensitive {
                line_text.clone()
            } else {
                line_text.to_lowercase()
            };

            let mut start = 0;
            while let Some(pos) = line_cmp[start..].find(&needle_cmp) {
                let abs_pos = start + pos;
                let end_pos = abs_pos + needle_cmp.len();

                if options.whole_word {
                    let before_ok = abs_pos == 0
                        || !line_text.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
                    let after_ok = end_pos >= line_text.len()
                        || !line_text.as_bytes()[end_pos].is_ascii_alphanumeric();
                    if before_ok && after_ok {
                        matches.push(SearchMatch {
                            row,
                            col_start: abs_pos,
                            col_end: end_pos,
                        });
                    }
                } else {
                    matches.push(SearchMatch {
                        row,
                        col_start: abs_pos,
                        col_end: end_pos,
                    });
                }
                start = abs_pos + 1;
            }
        }
    }
    matches
}

// ---------------------------------------------------------------------------
// Terminal selection
// ---------------------------------------------------------------------------

/// A rectangular or stream selection in the terminal buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl TerminalSelection {
    pub fn new(start_row: usize, start_col: usize, end_row: usize, end_col: usize) -> Self {
        // Normalize so start <= end
        if (start_row, start_col) <= (end_row, end_col) {
            Self {
                start_row,
                start_col,
                end_row,
                end_col,
            }
        } else {
            Self {
                start_row: end_row,
                start_col: end_col,
                end_row: start_row,
                end_col: start_col,
            }
        }
    }

    /// Check if a given cell position is inside this selection.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        if row < self.start_row || row > self.end_row {
            return false;
        }
        if row == self.start_row && row == self.end_row {
            return col >= self.start_col && col < self.end_col;
        }
        if row == self.start_row {
            return col >= self.start_col;
        }
        if row == self.end_row {
            return col < self.end_col;
        }
        true
    }

    /// Extract the selected text from a terminal buffer.
    pub fn extract_text(&self, buffer: &TerminalBuffer) -> String {
        let mut result = String::new();
        for row in self.start_row..=self.end_row {
            if let Some(cells) = buffer.line(row) {
                let col_start = if row == self.start_row {
                    self.start_col
                } else {
                    0
                };
                let col_end = if row == self.end_row {
                    self.end_col.min(cells.len())
                } else {
                    cells.len()
                };
                for col in col_start..col_end {
                    result.push(cells[col].ch);
                }
                // Trim trailing spaces on each line and add newline between rows
                if row < self.end_row {
                    let trimmed = result.trim_end_matches(' ');
                    result.truncate(trimmed.len());
                    result.push('\n');
                }
            }
        }
        // Trim trailing spaces from the last line
        let trimmed = result.trim_end_matches(' ');
        trimmed.to_string()
    }

    /// Returns true if the selection spans zero cells.
    pub fn is_empty(&self) -> bool {
        self.start_row == self.end_row && self.start_col == self.end_col
    }
}

// ---------------------------------------------------------------------------
// Terminal title tracker
// ---------------------------------------------------------------------------

/// Tracks the history of terminal title changes.
pub struct TitleTracker {
    history: Vec<String>,
    current: String,
    max_history: usize,
}

impl TitleTracker {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current: String::new(),
            max_history: 100,
        }
    }

    /// Update the current title, pushing the previous one to history.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let new = title.into();
        if new != self.current {
            if !self.current.is_empty() {
                self.history.push(self.current.clone());
                if self.history.len() > self.max_history {
                    self.history.remove(0);
                }
            }
            self.current = new;
        }
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Get the previous title (if any).
    pub fn previous(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Process ANSI actions and update the title if a SetTitle is found.
    pub fn process_actions(&mut self, actions: &[AnsiAction]) {
        for action in actions {
            if let AnsiAction::SetTitle(title) = action {
                self.set_title(title.clone());
            }
        }
    }
}

impl Default for TitleTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Terminal environment variable management
// ---------------------------------------------------------------------------

/// Manages environment variables for terminal sessions, supporting
/// variable inheritance, overrides, and shell-specific formatting.
pub struct TerminalEnv {
    base: HashMap<String, String>,
    overrides: HashMap<String, String>,
    removed: Vec<String>,
}

impl TerminalEnv {
    /// Create a new environment starting from the current process environment.
    pub fn from_current() -> Self {
        let base: HashMap<String, String> = env::vars().collect();
        Self {
            base,
            overrides: HashMap::new(),
            removed: Vec::new(),
        }
    }

    /// Create an empty environment.
    pub fn empty() -> Self {
        Self {
            base: HashMap::new(),
            overrides: HashMap::new(),
            removed: Vec::new(),
        }
    }

    /// Set or override an environment variable.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.removed.retain(|k| k != &key);
        self.overrides.insert(key, value.into());
    }

    /// Remove an environment variable.
    pub fn remove(&mut self, key: impl Into<String>) {
        let key = key.into();
        self.overrides.remove(&key);
        if !self.removed.contains(&key) {
            self.removed.push(key);
        }
    }

    /// Get a variable, checking overrides first, then base.
    pub fn get(&self, key: &str) -> Option<&str> {
        if self.removed.contains(&key.to_string()) {
            return None;
        }
        self.overrides
            .get(key)
            .or_else(|| self.base.get(key))
            .map(|s| s.as_str())
    }

    /// Build the final resolved environment as a HashMap.
    pub fn resolve(&self) -> HashMap<String, String> {
        let mut result = self.base.clone();
        for key in &self.removed {
            result.remove(key);
        }
        for (k, v) in &self.overrides {
            result.insert(k.clone(), v.clone());
        }
        result
    }

    /// Append a value to a PATH-style variable with the given separator.
    pub fn append_path(&mut self, key: &str, value: &str, separator: char) {
        let current = self.get(key).unwrap_or("").to_string();
        if current.is_empty() {
            self.set(key, value);
        } else {
            self.set(key, format!("{}{}{}", current, separator, value));
        }
    }

    /// Prepend a value to a PATH-style variable with the given separator.
    pub fn prepend_path(&mut self, key: &str, value: &str, separator: char) {
        let current = self.get(key).unwrap_or("").to_string();
        if current.is_empty() {
            self.set(key, value);
        } else {
            self.set(key, format!("{}{}{}", value, separator, current));
        }
    }

    /// Format a single variable as a shell `export` statement.
    pub fn format_export(&self, key: &str) -> Option<String> {
        self.get(key)
            .map(|v| format!("export {}=\"{}\"", key, v.replace('"', "\\\"")))
    }
}

// ---------------------------------------------------------------------------
// TerminalBuffer text extraction helpers
// ---------------------------------------------------------------------------

impl TerminalBuffer {
    /// Extract the text content of a single line (trailing spaces trimmed).
    pub fn line_text(&self, row: usize) -> Option<String> {
        self.line(row).map(|cells| {
            let s: String = cells.iter().map(|c| c.ch).collect();
            s.trim_end().to_string()
        })
    }

    /// Extract all visible text as a single string with newlines.
    pub fn visible_text(&self) -> String {
        self.visible_lines()
            .iter()
            .map(|line| {
                let s: String = line.iter().map(|c| c.ch).collect();
                s.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check whether the buffer contains the given text on any line.
    pub fn contains_text(&self, needle: &str) -> bool {
        for row in 0..self.line_count() {
            if let Some(text) = self.line_text(row) {
                if text.contains(needle) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the word at the given row and column position.
    pub fn word_at(&self, row: usize, col: usize) -> Option<String> {
        let cells = self.line(row)?;
        if col >= cells.len() || cells[col].ch.is_whitespace() {
            return None;
        }
        let mut start = col;
        while start > 0 && !cells[start - 1].ch.is_whitespace() {
            start -= 1;
        }
        let mut end = col;
        while end < cells.len() && !cells[end].ch.is_whitespace() {
            end += 1;
        }
        let word: String = cells[start..end].iter().map(|c| c.ch).collect();
        if word.is_empty() {
            None
        } else {
            Some(word)
        }
    }
}

// ---------------------------------------------------------------------------
// TerminalScrollbackBuffer – ring buffer for scrollback
// ---------------------------------------------------------------------------

/// A ring buffer that stores terminal scrollback lines.
#[derive(Debug, Clone)]
pub struct TerminalScrollbackBuffer {
    lines: Vec<String>,
    max_lines: usize,
    scroll_offset: usize,
    visible_count: usize,
}

impl TerminalScrollbackBuffer {
    pub fn new(max_lines: usize, visible_count: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
            scroll_offset: 0,
            visible_count,
        }
    }

    /// Push a new line into the buffer.
    pub fn push_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
        self.trim_to_max();
    }

    /// Get a line at a specific index.
    pub fn line_at(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    pub fn total_lines(&self) -> usize { self.lines.len() }

    /// Return visible lines from the current scroll offset.
    pub fn visible_lines(&self) -> Vec<&str> {
        self.lines
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .map(|s| s.as_str())
            .collect()
    }

    /// Set the scroll offset.
    pub fn scroll_to(&mut self, offset: usize) {
        self.scroll_offset = offset.min(self.lines.len().saturating_sub(self.visible_count));
    }

    /// Search for lines containing a pattern, returning their indices.
    pub fn search_lines(&self, pattern: &str) -> Vec<usize> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains(pattern))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    pub fn max_lines(&self) -> usize { self.max_lines }

    /// Trim buffer to max_lines capacity.
    pub fn trim_to_max(&mut self) {
        if self.lines.len() > self.max_lines {
            let excess = self.lines.len() - self.max_lines;
            self.lines.drain(0..excess);
            self.scroll_offset = self.scroll_offset.saturating_sub(excess);
        }
    }
}

// ---------------------------------------------------------------------------
// TerminalColorMapper – map 256-color palette to RGB
// ---------------------------------------------------------------------------

/// Maps terminal 256-color indices to RGB values.
#[derive(Debug, Clone)]
pub struct TerminalColorMapper {
    palette: [(u8, u8, u8); 16],
}

impl TerminalColorMapper {
    pub fn default_palette() -> Self {
        Self {
            palette: [
                (0, 0, 0),       // black
                (170, 0, 0),     // red
                (0, 170, 0),     // green
                (170, 85, 0),    // yellow/brown
                (0, 0, 170),     // blue
                (170, 0, 170),   // magenta
                (0, 170, 170),   // cyan
                (170, 170, 170), // white
                (85, 85, 85),    // bright black
                (255, 85, 85),   // bright red
                (85, 255, 85),   // bright green
                (255, 255, 85),  // bright yellow
                (85, 85, 255),   // bright blue
                (255, 85, 255),  // bright magenta
                (85, 255, 255),  // bright cyan
                (255, 255, 255), // bright white
            ],
        }
    }

    /// Map a color index (0-255) to an RGB tuple.
    pub fn color_index_to_rgb(&self, index: u8) -> (u8, u8, u8) {
        if index < 16 {
            self.palette[index as usize]
        } else if index < 232 {
            // 6x6x6 color cube
            let i = index - 16;
            let r = (i / 36) * 51;
            let g = ((i % 36) / 6) * 51;
            let b = (i % 6) * 51;
            (r, g, b)
        } else {
            // grayscale ramp
            let g = 8 + (index - 232) * 10;
            (g, g, g)
        }
    }

    /// Find the nearest 256-color index for an RGB value.
    pub fn nearest_256_color(&self, r: u8, g: u8, b: u8) -> u8 {
        let mut best_index = 0u8;
        let mut best_dist = u32::MAX;
        for i in 0..=255u8 {
            let (cr, cg, cb) = self.color_index_to_rgb(i);
            let dr = (r as i32 - cr as i32).unsigned_abs();
            let dg = (g as i32 - cg as i32).unsigned_abs();
            let db = (b as i32 - cb as i32).unsigned_abs();
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best_index = i;
            }
        }
        best_index
    }

    /// Check if a color index is a bright color (8-15).
    pub fn is_bright(&self, index: u8) -> bool {
        (8..16).contains(&index)
    }

    /// Get the bright variant of a base color (0-7 -> 8-15).
    pub fn bright_variant(&self, index: u8) -> u8 {
        if index < 8 { index + 8 } else { index }
    }
}

// ---------------------------------------------------------------------------
// AnsiSequenceParser – parse basic ANSI escape sequences
// ---------------------------------------------------------------------------

/// Parsed ANSI escape command.
#[derive(Debug, Clone, PartialEq)]
pub enum AnsiCommand {
    CursorUp(u32),
    CursorDown(u32),
    CursorForward(u32),
    CursorBack(u32),
    ClearScreen,
    SetColor(u8),
    ResetAttributes,
    Unknown(String),
}

/// Parse a CSI (Control Sequence Introducer) escape sequence.
pub fn parse_csi_sequence(params: &str, command: char) -> AnsiCommand {
    let n: u32 = params.parse().unwrap_or(1);
    match command {
        'A' => AnsiCommand::CursorUp(n),
        'B' => AnsiCommand::CursorDown(n),
        'C' => AnsiCommand::CursorForward(n),
        'D' => AnsiCommand::CursorBack(n),
        'J' => AnsiCommand::ClearScreen,
        'm' => {
            if params.is_empty() || params == "0" {
                AnsiCommand::ResetAttributes
            } else {
                AnsiCommand::SetColor(n as u8)
            }
        }
        _ => AnsiCommand::Unknown(format!("\\x1b[{}{}", params, command)),
    }
}

/// Extract the params and command character from an escape sequence body.
pub fn extract_csi_parts(seq: &str) -> Option<(&str, char)> {
    if seq.len() < 2 { return None; }
    let cmd = seq.chars().last()?;
    if cmd.is_ascii_alphabetic() {
        Some((&seq[..seq.len() - 1], cmd))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------



// ---------------------------------------------------------------------------
// terminal – Extended terminal bell state helpers
// ---------------------------------------------------------------------------

/// Priority levels for terminal bell state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZTerminalPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZTerminalPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZTerminalPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZTerminalPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks terminal bell state data.
#[derive(Debug, Clone)]
pub struct ZTerminalTerminalBellState {
    pub ring_times_ms: Vec<u64>,
    pub muted: bool,
    pub visual_bell: bool,
}

impl ZTerminalTerminalBellState {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            ring_times_ms: Vec::new(),
            muted: false,
            visual_bell: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.ring_times_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.ring_times_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.ring_times_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZTerminalTerminalBellState[muted={:?}, visual_bell={:?}]", self.muted, self.visual_bell)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.visual_bell = !c.visual_bell;
        c
    }
}

/// Compute a simple rolling hash for terminal bell state.
pub fn z_terminal_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_terminal_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_terminal_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_terminal_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_terminal_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_terminal_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_terminal_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 45
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer45 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer45 {
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
pub fn xb_fnv1a_45(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_45<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_45<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_45(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_45(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 178
// ---------------------------------------------------------------------------

/// Generic object pool `Xc178Pool<T>`.
pub struct Xc178Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc178Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc178PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc178Pool<T> {
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
    pub fn stats(&self) -> Xc178PoolStats {
        Xc178PoolStats {
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

impl<T> Default for Xc178Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc178Scheduler`.
pub struct Xc178Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc178Scheduler {
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

impl Default for Xc178Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_178 hash for the given byte slice.
pub fn xc_178_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_178 convention.
pub fn xc_178_reverse(s: &str) -> String {
    s.chars().rev().collect()
}

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

    // -- PtyConfig tests ---------------------------------------------------

    #[test]
    fn pty_config_default_values() {
        let config = PtyConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
        assert!(!config.shell.is_empty());
        assert!(config.cwd.is_none());
        assert!(config.env.is_empty());
    }

    #[test]
    fn pty_config_custom_values() {
        let config = PtyConfig {
            shell: "/bin/zsh".to_string(),
            cols: 120,
            rows: 40,
            cwd: Some(PathBuf::from("/tmp")),
            env: {
                let mut m = HashMap::new();
                m.insert("FOO".to_string(), "bar".to_string());
                m
            },
        };
        assert_eq!(config.shell, "/bin/zsh");
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
        assert_eq!(config.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(config.env.get("FOO").unwrap(), "bar");
    }

    #[test]
    fn pty_config_default_shell_matches_detect() {
        let config = PtyConfig::default();
        let detected = detect_default_shell();
        assert_eq!(config.shell, detected.to_string_lossy());
    }

    #[test]
    fn pty_config_clone() {
        let config = PtyConfig::default();
        let cloned = config.clone();
        assert_eq!(config.shell, cloned.shell);
        assert_eq!(config.cols, cloned.cols);
        assert_eq!(config.rows, cloned.rows);
    }

    #[test]
    fn detect_default_shell_non_empty() {
        let shell = detect_default_shell();
        assert!(!shell.to_string_lossy().is_empty());
    }

    // -- PtySession tests --------------------------------------------------

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_spawn_and_alive() {
        let mut pty = PtySession::spawn("/bin/sh", 80, 24).expect("spawn failed");
        assert!(pty.is_alive());
        pty.kill().expect("kill failed");
        // Wait for process to exit.
        let _ = pty.child.wait();
        assert!(!pty.is_alive());
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_write_input() {
        let mut pty = PtySession::spawn("/bin/sh", 80, 24).expect("spawn failed");
        let result = pty.write_input(b"echo hello\n");
        assert!(result.is_ok());
        pty.kill().expect("kill failed");
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_kill_lifecycle() {
        let mut pty = PtySession::spawn("/bin/sh", 80, 24).expect("spawn failed");
        assert!(pty.is_alive());
        assert!(pty.kill().is_ok());
        let _ = pty.child.wait();
        assert!(!pty.is_alive());
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_buffer_accessible() {
        let pty = PtySession::spawn("/bin/sh", 80, 24).expect("spawn failed");
        assert_eq!(pty.buffer().cols(), 80);
        assert_eq!(pty.buffer().rows(), 24);
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_buffer_mut_accessible() {
        let mut pty = PtySession::spawn("/bin/sh", 80, 24).expect("spawn failed");
        pty.buffer_mut().write_str("test");
        let line = pty.buffer().line(0).unwrap();
        assert_eq!(line[0].ch, 't');
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn pty_session_spawn_with_config() {
        let config = PtyConfig {
            shell: "/bin/sh".to_string(),
            cols: 100,
            rows: 30,
            cwd: Some(PathBuf::from("/tmp")),
            env: HashMap::new(),
        };
        let mut pty = PtySession::spawn_with_config(&config).expect("spawn_with_config failed");
        assert!(pty.is_alive());
        assert_eq!(pty.buffer().cols(), 100);
        assert_eq!(pty.buffer().rows(), 30);
        pty.kill().expect("kill failed");
    }

    #[test]
    fn pty_session_spawn_invalid_shell() {
        let result = PtySession::spawn("/nonexistent/shell", 80, 24);
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // Spawns real process; may be flaky in CI
    fn service_create_with_pty() {
        let mut svc = TerminalService::new();
        let id = svc.create_with_pty(Some("/bin/sh"), 80, 24).expect("create_with_pty failed");
        assert!(svc.get(id).is_some());
        assert_eq!(svc.count(), 1);
    }

    #[test]
    fn service_create_with_pty_invalid_shell() {
        let mut svc = TerminalService::new();
        let result = svc.create_with_pty(Some("/nonexistent/shell"), 80, 24);
        assert!(result.is_err());
        assert_eq!(svc.count(), 0);
    }

    // -- Link detection tests -----------------------------------------------

    #[test]
    fn detect_url_link() {
        let mut buf = TerminalBuffer::new(120, 24);
        buf.write_str("Visit https://example.com/path for info");
        let links = detect_links(&buf);
        assert!(links.iter().any(|l| l.kind == LinkKind::Url
            && l.target == "https://example.com/path"));
    }

    #[test]
    fn detect_http_url() {
        let mut buf = TerminalBuffer::new(120, 24);
        buf.write_str("Go to http://localhost:8080/api/test now");
        let links = detect_links(&buf);
        assert!(links.iter().any(|l| l.kind == LinkKind::Url
            && l.target == "http://localhost:8080/api/test"));
    }

    #[test]
    fn detect_file_path_link() {
        let mut buf = TerminalBuffer::new(120, 24);
        buf.write_str("error at src/main.rs:42");
        let links = detect_links(&buf);
        assert!(links.iter().any(|l| l.kind == LinkKind::FilePath
            && l.target.contains("src/main.rs:42")));
    }

    #[test]
    fn detect_file_path_with_col() {
        let mut buf = TerminalBuffer::new(120, 24);
        buf.write_str("warning: src/lib.rs:10:5 unused variable");
        let links = detect_links(&buf);
        assert!(links.iter().any(|l| l.kind == LinkKind::FilePath
            && l.target.contains("src/lib.rs:10:5")));
    }

    #[test]
    fn detect_no_links_in_plain_text() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello world, nothing special here");
        let links = detect_links(&buf);
        assert!(links.is_empty());
    }

    // -- Search tests -------------------------------------------------------

    #[test]
    fn search_case_insensitive() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello World");
        let matches = search_buffer(&buf, "hello", &SearchOptions::default());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col_start, 0);
        assert_eq!(matches[0].col_end, 5);
    }

    #[test]
    fn search_case_sensitive() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello World");
        let opts = SearchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let matches = search_buffer(&buf, "hello", &opts);
        assert!(matches.is_empty());
        let matches2 = search_buffer(&buf, "Hello", &opts);
        assert_eq!(matches2.len(), 1);
    }

    #[test]
    fn search_whole_word() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("foobar foo barfoo");
        let opts = SearchOptions {
            whole_word: true,
            ..Default::default()
        };
        let matches = search_buffer(&buf, "foo", &opts);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col_start, 7);
    }

    #[test]
    fn search_multiple_matches() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("abcabcabc");
        let matches = search_buffer(&buf, "abc", &SearchOptions::default());
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn search_empty_needle() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello");
        let matches = search_buffer(&buf, "", &SearchOptions::default());
        assert!(matches.is_empty());
    }

    // -- Selection tests ----------------------------------------------------

    #[test]
    fn selection_contains() {
        let sel = TerminalSelection::new(1, 5, 3, 10);
        assert!(!sel.contains(0, 5));
        assert!(sel.contains(1, 5));
        assert!(sel.contains(1, 20));
        assert!(sel.contains(2, 0));
        assert!(sel.contains(3, 9));
        assert!(!sel.contains(3, 10));
        assert!(!sel.contains(4, 0));
    }

    #[test]
    fn selection_single_row() {
        let sel = TerminalSelection::new(2, 3, 2, 8);
        assert!(!sel.contains(2, 2));
        assert!(sel.contains(2, 3));
        assert!(sel.contains(2, 7));
        assert!(!sel.contains(2, 8));
    }

    #[test]
    fn selection_extract_text() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello World");
        let sel = TerminalSelection::new(0, 0, 0, 5);
        assert_eq!(sel.extract_text(&buf), "Hello");
    }

    #[test]
    fn selection_normalizes_reverse() {
        let sel = TerminalSelection::new(5, 10, 2, 3);
        assert_eq!(sel.start_row, 2);
        assert_eq!(sel.start_col, 3);
        assert_eq!(sel.end_row, 5);
        assert_eq!(sel.end_col, 10);
    }

    #[test]
    fn selection_is_empty() {
        let sel = TerminalSelection::new(3, 5, 3, 5);
        assert!(sel.is_empty());
        let sel2 = TerminalSelection::new(3, 5, 3, 6);
        assert!(!sel2.is_empty());
    }

    // -- Title tracker tests ------------------------------------------------

    #[test]
    fn title_tracker_set_and_history() {
        let mut tracker = TitleTracker::new();
        assert_eq!(tracker.current(), "");
        tracker.set_title("First");
        assert_eq!(tracker.current(), "First");
        assert!(tracker.history().is_empty());
        tracker.set_title("Second");
        assert_eq!(tracker.current(), "Second");
        assert_eq!(tracker.previous(), Some("First"));
        tracker.set_title("Third");
        assert_eq!(tracker.history().len(), 2);
    }

    #[test]
    fn title_tracker_duplicate_no_push() {
        let mut tracker = TitleTracker::new();
        tracker.set_title("Same");
        tracker.set_title("Same");
        assert!(tracker.history().is_empty());
    }

    #[test]
    fn title_tracker_process_actions() {
        let mut tracker = TitleTracker::new();
        let actions = vec![
            AnsiAction::Print('x'),
            AnsiAction::SetTitle("My Terminal".into()),
            AnsiAction::Print('y'),
        ];
        tracker.process_actions(&actions);
        assert_eq!(tracker.current(), "My Terminal");
    }

    // -- Terminal environment tests -----------------------------------------

    #[test]
    fn terminal_env_set_and_get() {
        let mut env = TerminalEnv::empty();
        env.set("FOO", "bar");
        assert_eq!(env.get("FOO"), Some("bar"));
        assert_eq!(env.get("MISSING"), None);
    }

    #[test]
    fn terminal_env_remove() {
        let mut env = TerminalEnv::empty();
        env.set("KEY", "value");
        env.remove("KEY");
        assert_eq!(env.get("KEY"), None);
    }

    #[test]
    fn terminal_env_override_base() {
        let mut env = TerminalEnv::empty();
        // Simulate a base var
        env.base.insert("LANG".into(), "en_US".into());
        assert_eq!(env.get("LANG"), Some("en_US"));
        env.set("LANG", "C");
        assert_eq!(env.get("LANG"), Some("C"));
    }

    #[test]
    fn terminal_env_resolve() {
        let mut env = TerminalEnv::empty();
        env.base.insert("A".into(), "1".into());
        env.base.insert("B".into(), "2".into());
        env.set("C", "3");
        env.remove("B");
        let resolved = env.resolve();
        assert_eq!(resolved.get("A").unwrap(), "1");
        assert!(!resolved.contains_key("B"));
        assert_eq!(resolved.get("C").unwrap(), "3");
    }

    #[test]
    fn terminal_env_append_path() {
        let mut env = TerminalEnv::empty();
        env.set("PATH", "/usr/bin");
        env.append_path("PATH", "/home/bin", ':');
        assert_eq!(env.get("PATH"), Some("/usr/bin:/home/bin"));
    }

    #[test]
    fn terminal_env_prepend_path() {
        let mut env = TerminalEnv::empty();
        env.set("PATH", "/usr/bin");
        env.prepend_path("PATH", "/home/bin", ':');
        assert_eq!(env.get("PATH"), Some("/home/bin:/usr/bin"));
    }

    #[test]
    fn terminal_env_format_export() {
        let mut env = TerminalEnv::empty();
        env.set("MY_VAR", "hello world");
        assert_eq!(
            env.format_export("MY_VAR"),
            Some("export MY_VAR=\"hello world\"".to_string())
        );
        assert_eq!(env.format_export("NOPE"), None);
    }

    // -- Buffer text extraction tests ---------------------------------------

    #[test]
    fn buffer_line_text() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("Hello World");
        assert_eq!(buf.line_text(0), Some("Hello World".to_string()));
    }

    #[test]
    fn buffer_contains_text() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("error: file not found");
        assert!(buf.contains_text("not found"));
        assert!(!buf.contains_text("success"));
    }

    #[test]
    fn buffer_visible_text() {
        let mut buf = TerminalBuffer::new(20, 3);
        buf.write_str("AAA\r\nBBB\r\nCCC");
        let text = buf.visible_text();
        assert!(text.contains("AAA"));
        assert!(text.contains("BBB"));
        assert!(text.contains("CCC"));
    }

    #[test]
    fn buffer_word_at() {
        let mut buf = TerminalBuffer::new(80, 24);
        buf.write_str("hello world foo");
        assert_eq!(buf.word_at(0, 0), Some("hello".to_string()));
        assert_eq!(buf.word_at(0, 6), Some("world".to_string()));
        assert_eq!(buf.word_at(0, 5), None); // space
    }

    #[test]
    fn buffer_word_at_out_of_bounds() {
        let buf = TerminalBuffer::new(80, 24);
        assert_eq!(buf.word_at(0, 999), None);
    }

    // -- TerminalScrollbackBuffer -------------------------------------------

    #[test]
    fn scrollback_push_and_get() {
        let mut sb = TerminalScrollbackBuffer::new(100, 5);
        sb.push_line("hello");
        sb.push_line("world");
        assert_eq!(sb.total_lines(), 2);
        assert_eq!(sb.line_at(0), Some("hello"));
        assert_eq!(sb.line_at(1), Some("world"));
    }

    #[test]
    fn scrollback_visible_lines() {
        let mut sb = TerminalScrollbackBuffer::new(100, 2);
        sb.push_line("a");
        sb.push_line("b");
        sb.push_line("c");
        let vis = sb.visible_lines();
        assert_eq!(vis, vec!["a", "b"]);
    }

    #[test]
    fn scrollback_scroll_to() {
        let mut sb = TerminalScrollbackBuffer::new(100, 2);
        sb.push_line("a");
        sb.push_line("b");
        sb.push_line("c");
        sb.scroll_to(1);
        let vis = sb.visible_lines();
        assert_eq!(vis, vec!["b", "c"]);
    }

    #[test]
    fn scrollback_trim_to_max() {
        let mut sb = TerminalScrollbackBuffer::new(3, 2);
        for i in 0..5 {
            sb.push_line(&format!("line{}", i));
        }
        assert_eq!(sb.total_lines(), 3);
        assert_eq!(sb.line_at(0), Some("line2"));
    }

    #[test]
    fn scrollback_search() {
        let mut sb = TerminalScrollbackBuffer::new(100, 10);
        sb.push_line("error: something failed");
        sb.push_line("info: all good");
        sb.push_line("error: another failure");
        let results = sb.search_lines("error");
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn scrollback_clear() {
        let mut sb = TerminalScrollbackBuffer::new(100, 10);
        sb.push_line("data");
        sb.clear();
        assert_eq!(sb.total_lines(), 0);
    }

    // -- TerminalColorMapper ------------------------------------------------

    #[test]
    fn color_mapper_basic_colors() {
        let cm = TerminalColorMapper::default_palette();
        assert_eq!(cm.color_index_to_rgb(0), (0, 0, 0));
        assert_eq!(cm.color_index_to_rgb(15), (255, 255, 255));
    }

    #[test]
    fn color_mapper_is_bright() {
        let cm = TerminalColorMapper::default_palette();
        assert!(!cm.is_bright(0));
        assert!(cm.is_bright(8));
        assert!(cm.is_bright(15));
        assert!(!cm.is_bright(16));
    }

    #[test]
    fn color_mapper_bright_variant() {
        let cm = TerminalColorMapper::default_palette();
        assert_eq!(cm.bright_variant(1), 9);
        assert_eq!(cm.bright_variant(10), 10);
    }

    #[test]
    fn color_mapper_nearest() {
        let cm = TerminalColorMapper::default_palette();
        let idx = cm.nearest_256_color(0, 0, 0);
        assert_eq!(idx, 0);
    }

    // -- AnsiSequenceParser -------------------------------------------------

    #[test]
    fn parse_csi_cursor_up() {
        assert_eq!(parse_csi_sequence("3", 'A'), AnsiCommand::CursorUp(3));
    }

    #[test]
    fn parse_csi_reset() {
        assert_eq!(parse_csi_sequence("0", 'm'), AnsiCommand::ResetAttributes);
        assert_eq!(parse_csi_sequence("", 'm'), AnsiCommand::ResetAttributes);
    }

    #[test]
    fn extract_csi_parts_valid() {
        let (params, cmd) = extract_csi_parts("3A").unwrap();
        assert_eq!(params, "3");
        assert_eq!(cmd, 'A');
    }

    // -- terminal Z-extended tests -----------------------------------------------

    #[test]
    fn z_terminal_priority_weight() {
        assert_eq!(ZTerminalPriority::Idle.weight(), 0);
        assert_eq!(ZTerminalPriority::Normal.weight(), 2);
        assert_eq!(ZTerminalPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_terminal_priority_label() {
        assert_eq!(ZTerminalPriority::Low.label(), "low");
        assert_eq!(ZTerminalPriority::High.label(), "high");
    }

    #[test]
    fn z_terminal_priority_is_elevated() {
        assert!(!ZTerminalPriority::Normal.is_elevated());
        assert!(ZTerminalPriority::High.is_elevated());
        assert!(ZTerminalPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_terminal_priority_display() {
        assert_eq!(format!("{}", ZTerminalPriority::Idle), "idle");
    }

    #[test]
    fn z_terminal_priority_all_asc() {
        let all = ZTerminalPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZTerminalPriority::Idle);
        assert_eq!(all[4], ZTerminalPriority::Realtime);
    }

    #[test]
    fn z_terminal_struct_new() {
        let s = ZTerminalTerminalBellState::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_terminal_struct_toggled_clone() {
        let s = ZTerminalTerminalBellState::new();
        let t = s.toggled_clone();
        assert_ne!(s.visual_bell, t.visual_bell);
    }

    #[test]
    fn z_terminal_rolling_hash_deterministic() {
        let h1 = z_terminal_rolling_hash(b"test");
        let h2 = z_terminal_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_terminal_rolling_hash(b"a"), z_terminal_rolling_hash(b"b"));
    }

    #[test]
    fn z_terminal_pad_to_basic() {
        assert_eq!(z_terminal_pad_to("hi", 5), "hi   ");
        assert_eq!(z_terminal_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_terminal_is_identifier_basic() {
        assert!(z_terminal_is_identifier("foo_bar"));
        assert!(z_terminal_is_identifier("abc123"));
        assert!(!z_terminal_is_identifier(""));
        assert!(!z_terminal_is_identifier("has space"));
    }

    #[test]
    fn z_terminal_levenshtein_basic() {
        assert_eq!(z_terminal_levenshtein("", ""), 0);
        assert_eq!(z_terminal_levenshtein("abc", "abc"), 0);
        assert_eq!(z_terminal_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_terminal_unique_words_basic() {
        let w = z_terminal_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_terminal_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_terminal_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_terminal_common_prefix_basic() {
        assert_eq!(z_terminal_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_terminal_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_terminal_struct_clear() {
        let mut s = ZTerminalTerminalBellState::new();
        s.ring_times_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_terminal_rolling_hash_empty() {
        let h = z_terminal_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_45_push_and_len() {
        let mut rb = super::XbRingBuffer45::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_45_overwrite() {
        let mut rb = super::XbRingBuffer45::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_45_get_out_of_bounds() {
        let rb = super::XbRingBuffer45::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_45_drain_all() {
        let mut rb = super::XbRingBuffer45::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_45_peek_front_back() {
        let mut rb = super::XbRingBuffer45::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_45_clear() {
        let mut rb = super::XbRingBuffer45::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_45_capacity() {
        let rb = super::XbRingBuffer45::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_45_basic() {
        let h = super::xb_fnv1a_45(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_45(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_45_different_inputs() {
        let h1 = super::xb_fnv1a_45(b"abc");
        let h2 = super::xb_fnv1a_45(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_45_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_45(&data);
        let dec = super::xb_rle_decode_45(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_45_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_45(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_45(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_45_values() {
        assert!((super::xb_clamp_45(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_45(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_45(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_45_values() {
        assert!((super::xb_lerp_45(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_45(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_45(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_45_wrap_around_twice() {
        let mut rb = super::XbRingBuffer45::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 178 ----

    #[test]
    fn xc_178_pool_new_empty() {
        let pool: super::Xc178Pool<i32> = super::Xc178Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_178_pool_release_acquire() {
        let mut pool = super::Xc178Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_178_pool_acquire_empty() {
        let mut pool: super::Xc178Pool<i32> = super::Xc178Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_178_pool_full() {
        let mut pool = super::Xc178Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_178_pool_drain() {
        let mut pool = super::Xc178Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_178_pool_stats() {
        let mut pool = super::Xc178Pool::new(8);
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
    fn xc_178_pool_clear() {
        let mut pool = super::Xc178Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_178_pool_shrink() {
        let mut pool = super::Xc178Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_178_pool_default() {
        let pool: super::Xc178Pool<String> = super::Xc178Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_178_pool_extend() {
        let mut pool = super::Xc178Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_178_pool_retain() {
        let mut pool = super::Xc178Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_178_scheduler_round_robin() {
        let mut sched = super::Xc178Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_178_scheduler_empty() {
        let mut sched = super::Xc178Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_178_scheduler_reset() {
        let mut sched = super::Xc178Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_178_scheduler_add_remove() {
        let mut sched = super::Xc178Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_178_scheduler_targets() {
        let sched = super::Xc178Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_178_hash_empty() {
        assert_eq!(super::xc_178_hash(b""), 5381);
    }

    #[test]
    fn xc_178_hash_data() {
        let h = super::xc_178_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_178_hash(b"hello"), h);
    }

    #[test]
    fn xc_178_reverse_str() {
        assert_eq!(super::xc_178_reverse("abc"), "cba");
        assert_eq!(super::xc_178_reverse(""), "");
    }

}
