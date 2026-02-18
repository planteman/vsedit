//! Terminal UI framework core (Ratatui + Crossterm).
//!
//! This crate provides the foundational TUI layer for vsedit, wrapping Ratatui
//! and Crossterm to handle terminal setup/teardown, the async event loop, and
//! rendering. It replaces Electron's browser-based rendering.
//!
//! # Key types
//!
//! - [`App`] — the main application loop that owns the terminal and drives
//!   rendering.
//! - [`AppEvent`] — events produced by the event loop (keys, mouse, resize,
//!   paste, ticks, quit).
//! - [`setup_terminal`] / [`restore_terminal`] — raw mode, alternate screen,
//!   mouse capture, and bracketed paste management.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> std::io::Result<()> {
//! use vsedit_tui::App;
//!
//! let mut app = App::new()?;
//! app.run(|frame| {
//!     // draw widgets into `frame`
//! }).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
    EnableMouseCapture, Event as CtEvent, EventStream, KeyCode, KeyEvent,
    KeyModifiers, MouseEvent,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

// ---------------------------------------------------------------------------
// Re-exports — convenience access to common ratatui types
// ---------------------------------------------------------------------------

pub use ratatui::Frame;
pub use ratatui::layout::Rect;
pub use ratatui::style::{Color, Modifier, Style};
pub use ratatui::text::{Line, Span, Text};

// ---------------------------------------------------------------------------
// AppEvent
// ---------------------------------------------------------------------------

/// Events produced by the TUI event loop.
#[derive(Clone, Debug)]
pub enum AppEvent {
    /// A keyboard event.
    Key(KeyEvent),
    /// A mouse event.
    Mouse(MouseEvent),
    /// The terminal was resized to `(columns, rows)`.
    Resize(u16, u16),
    /// Text pasted via bracketed paste.
    Paste(String),
    /// A periodic tick (driven by the configured frame rate).
    Tick,
    /// A request to shut down the application.
    Quit,
}

// ---------------------------------------------------------------------------
// Terminal setup / teardown
// ---------------------------------------------------------------------------

/// Prepare the terminal for TUI rendering.
///
/// Enables raw mode, switches to the alternate screen, turns on mouse capture,
/// and enables bracketed paste. Call [`restore_terminal`] (or drop [`App`]) to
/// undo these changes.
pub fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state.
///
/// Disables bracketed paste, mouse capture, and raw mode, then leaves the
/// alternate screen. Safe to call multiple times.
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Install a panic hook that restores the terminal before printing the panic.
///
/// This prevents the user's shell from being left in a broken state when an
/// unrecoverable error occurs.
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration — ignore errors.
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        original_hook(panic_info);
    }));
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Default tick interval (~60 fps).
const DEFAULT_TICK_RATE: Duration = Duration::from_millis(16);

/// The main TUI application driver.
///
/// `App` owns the [`Terminal`], manages the async event loop, and coordinates
/// rendering. Create one with [`App::new`], then call [`App::run`] with a
/// render callback.
pub struct App {
    /// The ratatui terminal handle.
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Shared flag signalling that the application should exit.
    should_quit: Arc<AtomicBool>,
    /// Duration between tick events.
    tick_rate: Duration,
}

impl App {
    /// Create a new `App`, setting up the terminal and installing the panic
    /// hook.
    pub fn new() -> io::Result<Self> {
        install_panic_hook();
        let terminal = setup_terminal()?;
        Ok(Self {
            terminal,
            should_quit: Arc::new(AtomicBool::new(false)),
            tick_rate: DEFAULT_TICK_RATE,
        })
    }

    /// Override the tick rate (default ≈ 60 fps / 16 ms).
    pub fn with_tick_rate(mut self, rate: Duration) -> Self {
        self.tick_rate = rate;
        self
    }

    /// Signal the application to quit after the current frame.
    pub fn quit(&self) {
        self.should_quit.store(true, Ordering::SeqCst);
    }

    /// Run the main event + render loop.
    ///
    /// `render` is called once per frame with a mutable [`Frame`] reference.
    /// The loop exits when [`App::quit`] is called or `Ctrl+C` is pressed.
    pub async fn run<F>(&mut self, mut render: F) -> io::Result<()>
    where
        F: FnMut(&mut Frame),
    {
        let mut event_stream = EventStream::new();
        let mut tick_interval = tokio::time::interval(self.tick_rate);

        while !self.should_quit.load(Ordering::SeqCst) {
            // Draw a frame.
            self.draw(&mut render)?;

            // Wait for the next event or tick.
            let event = tokio::select! {
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(ev)) => self.translate_event(ev),
                        Some(Err(_)) => AppEvent::Quit,
                        None => AppEvent::Quit,
                    }
                }
                _ = tick_interval.tick() => AppEvent::Tick,
            };

            if matches!(event, AppEvent::Quit) {
                break;
            }
        }

        restore_terminal(&mut self.terminal)?;
        Ok(())
    }

    /// Perform a single render cycle: clear dirty regions, invoke the user
    /// callback, and flush to the terminal.
    pub fn draw<F>(&mut self, render: &mut F) -> io::Result<()>
    where
        F: FnMut(&mut Frame),
    {
        self.terminal.draw(|frame| render(frame))?;
        self.terminal.backend_mut().flush()?;
        Ok(())
    }

    /// Convert a crossterm [`Event`](CtEvent) into an [`AppEvent`].
    fn translate_event(&self, event: CtEvent) -> AppEvent {
        match event {
            CtEvent::Key(key) => {
                // Ctrl+C ⇒ graceful shutdown.
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    self.quit();
                    return AppEvent::Quit;
                }
                AppEvent::Key(key)
            }
            CtEvent::Mouse(mouse) => AppEvent::Mouse(mouse),
            CtEvent::Resize(cols, rows) => AppEvent::Resize(cols, rows),
            CtEvent::Paste(text) => AppEvent::Paste(text),
            CtEvent::FocusGained | CtEvent::FocusLost => AppEvent::Tick,
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = restore_terminal(&mut self.terminal);
    }
}

// ---------------------------------------------------------------------------
// StatusBar — a simple status bar model for the TUI
// ---------------------------------------------------------------------------

/// Represents a status bar item position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAlignment {
    Left,
    Right,
}

/// A single item shown in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub alignment: StatusBarAlignment,
    pub priority: i32,
}

impl StatusBarItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: None,
            alignment: StatusBarAlignment::Left,
            priority: 0,
        }
    }

    pub fn with_alignment(mut self, alignment: StatusBarAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl fmt::Display for StatusBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

/// Manages status bar items.
#[derive(Debug, Default)]
pub struct StatusBar {
    items: Vec<StatusBarItem>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add_item(&mut self, item: StatusBarItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != before
    }

    pub fn update_text(&mut self, id: &str, text: impl Into<String>) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.text = text.into();
            true
        } else {
            false
        }
    }

    pub fn get_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Return left-aligned items sorted by priority (highest first).
    pub fn left_items(&self) -> Vec<&StatusBarItem> {
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.alignment == StatusBarAlignment::Left)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items
    }

    /// Return right-aligned items sorted by priority (highest first).
    pub fn right_items(&self) -> Vec<&StatusBarItem> {
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.alignment == StatusBarAlignment::Right)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Render the status bar as a single string: left items | right items.
    pub fn render_text(&self, width: usize) -> String {
        let left: String = self
            .left_items()
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        let right: String = self
            .right_items()
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>()
            .join(" | ");

        let padding = if left.len() + right.len() < width {
            width - left.len() - right.len()
        } else {
            1
        };
        format!("{}{}{}", left, " ".repeat(padding), right)
    }
}

// ---------------------------------------------------------------------------
// KeyBinding helpers
// ---------------------------------------------------------------------------

/// Represents a parsed key chord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

impl KeyChord {
    /// Parse a key chord string like "Ctrl+Shift+S".
    pub fn parse(input: &str) -> Option<Self> {
        let parts: Vec<&str> = input.split('+').collect();
        if parts.is_empty() {
            return None;
        }
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut key = String::new();

        for (i, part) in parts.iter().enumerate() {
            let lower = part.to_lowercase();
            if i < parts.len() - 1 {
                match lower.as_str() {
                    "ctrl" => ctrl = true,
                    "alt" => alt = true,
                    "shift" => shift = true,
                    _ => return None,
                }
            } else {
                key = part.to_string();
            }
        }

        if key.is_empty() {
            return None;
        }

        Some(Self {
            ctrl,
            alt,
            shift,
            key,
        })
    }

    /// Check if this chord matches a crossterm KeyEvent.
    pub fn matches_key_event(&self, event: &KeyEvent) -> bool {
        let mods = event.modifiers;
        if self.ctrl != mods.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if self.alt != mods.contains(KeyModifiers::ALT) {
            return false;
        }
        if self.shift != mods.contains(KeyModifiers::SHIFT) {
            return false;
        }
        match event.code {
            KeyCode::Char(c) => {
                self.key.len() == 1
                    && self
                        .key
                        .chars()
                        .next()
                        .map(|k| k.to_lowercase().eq(c.to_lowercase()))
                        .unwrap_or(false)
            }
            KeyCode::F(n) => self.key == format!("F{n}"),
            KeyCode::Enter => self.key.eq_ignore_ascii_case("Enter"),
            KeyCode::Esc => self.key.eq_ignore_ascii_case("Escape") || self.key.eq_ignore_ascii_case("Esc"),
            KeyCode::Backspace => self.key.eq_ignore_ascii_case("Backspace"),
            KeyCode::Tab => self.key.eq_ignore_ascii_case("Tab"),
            KeyCode::Delete => self.key.eq_ignore_ascii_case("Delete"),
            _ => false,
        }
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(&self.key);
        write!(f, "{}", parts.join("+"))
    }
}

// ---------------------------------------------------------------------------
// Terminal capability detection
// ---------------------------------------------------------------------------

/// Detected terminal capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub supports_color: bool,
    pub supports_256_color: bool,
    pub supports_true_color: bool,
    pub supports_mouse: bool,
    pub supports_bracketed_paste: bool,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            supports_color: true,
            supports_256_color: true,
            supports_true_color: false,
            supports_mouse: true,
            supports_bracketed_paste: true,
        }
    }
}

/// Detect terminal capabilities from environment variables.
pub fn detect_capabilities() -> TerminalCapabilities {
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    let supports_true_color =
        colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit");
    let supports_256 = supports_true_color || term.contains("256color");
    TerminalCapabilities {
        supports_color: !term.is_empty(),
        supports_256_color: supports_256,
        supports_true_color,
        supports_mouse: true,
        supports_bracketed_paste: true,
    }
}

// ---------------------------------------------------------------------------
// Color theme management
// ---------------------------------------------------------------------------

/// A named color theme with foreground/background pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorTheme {
    pub name: String,
    pub fg: String,
    pub bg: String,
    pub accent: String,
}

/// Manages a collection of color themes.
#[derive(Debug, Default)]
pub struct ThemeManager {
    themes: Vec<ColorTheme>,
    active: Option<String>,
}

impl ThemeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_theme(&mut self, theme: ColorTheme) {
        self.themes.push(theme);
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if self.themes.iter().any(|t| t.name == name) {
            self.active = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn active_theme(&self) -> Option<&ColorTheme> {
        self.active
            .as_ref()
            .and_then(|n| self.themes.iter().find(|t| &t.name == n))
    }

    pub fn theme_names(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.name.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Layout constraint computation
// ---------------------------------------------------------------------------

/// A layout constraint for a TUI panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutConstraint {
    Fixed(u16),
    Percentage(f32),
    Min(u16),
    Max(u16),
}

/// Resolve a sequence of constraints into concrete sizes for a total available space.
pub fn resolve_constraints(constraints: &[LayoutConstraint], total: u16) -> Vec<u16> {
    constraints
        .iter()
        .map(|c| match c {
            LayoutConstraint::Fixed(v) => *v,
            LayoutConstraint::Percentage(p) => ((total as f32) * p / 100.0) as u16,
            LayoutConstraint::Min(v) => *v,
            LayoutConstraint::Max(v) => (*v).min(total),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Render statistics tracking
// ---------------------------------------------------------------------------

/// Statistics for a single render frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderStats {
    pub frame_number: u64,
    pub render_time_us: u64,
    pub widget_count: u32,
}

/// Tracks render statistics across frames.
#[derive(Debug, Default)]
pub struct RenderTracker {
    frames: Vec<RenderStats>,
}

impl RenderTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, stats: RenderStats) {
        self.frames.push(stats);
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn average_render_time_us(&self) -> u64 {
        if self.frames.is_empty() {
            return 0;
        }
        let total: u64 = self.frames.iter().map(|f| f.render_time_us).sum();
        total / self.frames.len() as u64
    }

    pub fn last_frame(&self) -> Option<&RenderStats> {
        self.frames.last()
    }
}

// ---------------------------------------------------------------------------
// Extended TerminalCapabilities methods
// ---------------------------------------------------------------------------

impl TerminalCapabilities {
    /// Placeholder: sixel graphics are not yet detected.
    pub fn supports_sixel(&self) -> bool {
        false
    }

    /// Placeholder: Kitty graphics protocol is not yet detected.
    pub fn supports_kitty_graphics(&self) -> bool {
        false
    }

    /// Returns a human-readable summary of detected capabilities.
    pub fn capability_summary(&self) -> String {
        format!(
            "truecolor: {}, 256color: {}, mouse: {}, bracketed_paste: {}, sixel: {}, kitty_graphics: {}",
            if self.supports_true_color { "yes" } else { "no" },
            if self.supports_256_color { "yes" } else { "no" },
            if self.supports_mouse { "yes" } else { "no" },
            if self.supports_bracketed_paste { "yes" } else { "no" },
            if self.supports_sixel() { "yes" } else { "no" },
            if self.supports_kitty_graphics() { "yes" } else { "no" },
        )
    }

    /// Detect capabilities from environment variables.
    pub fn from_env() -> Self {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let term = std::env::var("TERM").unwrap_or_default();
        let supports_true_color =
            colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit");
        let supports_256 = supports_true_color || term.contains("256color");
        Self {
            supports_color: !term.is_empty() || !colorterm.is_empty(),
            supports_256_color: supports_256,
            supports_true_color,
            supports_mouse: true,
            supports_bracketed_paste: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Terminal size helper
// ---------------------------------------------------------------------------

/// Returns the current terminal size as `(columns, rows)`.
///
/// Falls back to `(80, 24)` if the size cannot be determined.
pub fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

// ---------------------------------------------------------------------------
// RenderRegion — partial screen update with dirty tracking
// ---------------------------------------------------------------------------

/// A rectangular screen region with dirty tracking for partial updates.
#[derive(Debug, Clone)]
pub struct RenderRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub dirty: bool,
    pub content: Vec<String>,
}

impl RenderRegion {
    /// Create a new render region. Content is initialized to empty strings.
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
            dirty: true,
            content: vec![String::new(); height as usize],
        }
    }

    /// Mark the region as needing a redraw.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Mark the region as up-to-date.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Returns whether the region needs a redraw.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Set the content for a given row (0-based) and mark dirty.
    pub fn set_line(&mut self, row: usize, text: String) {
        if row < self.content.len() {
            self.content[row] = text;
            self.dirty = true;
        }
    }

    /// Get the content for a given row.
    pub fn get_line(&self, row: usize) -> Option<&str> {
        self.content.get(row).map(|s| s.as_str())
    }

    /// Clear all content and mark dirty.
    pub fn clear(&mut self) {
        for line in &mut self.content {
            line.clear();
        }
        self.dirty = true;
    }

    /// Check if this region overlaps with another.
    pub fn overlaps(&self, other: &RenderRegion) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Check if a point falls within this region.
    pub fn contains_point(&self, px: u16, py: u16) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }
}

// ---------------------------------------------------------------------------
// KeyChord extensions
// ---------------------------------------------------------------------------

impl KeyChord {
    pub fn is_modifier_only(&self) -> bool {
        self.key.is_empty()
    }

    pub fn modifier_count(&self) -> usize {
        self.ctrl as usize + self.alt as usize + self.shift as usize
    }

    pub fn has_modifiers(&self) -> bool {
        self.ctrl || self.alt || self.shift
    }
}

// ---------------------------------------------------------------------------
// TerminalCapabilities summary extension
// ---------------------------------------------------------------------------

impl TerminalCapabilities {
    pub fn enabled_count(&self) -> usize {
        self.supports_color as usize
            + self.supports_256_color as usize
            + self.supports_true_color as usize
            + self.supports_mouse as usize
            + self.supports_bracketed_paste as usize
    }
}

// ---------------------------------------------------------------------------
// ColorTheme extensions
// ---------------------------------------------------------------------------

impl ColorTheme {
    pub fn color_count(&self) -> usize {
        3
    }

    pub fn has_color(&self, hex: &str) -> bool {
        self.fg == hex || self.bg == hex || self.accent == hex
    }

    pub fn is_dark(&self) -> bool {
        parse_hex_luminance(&self.bg).map_or(false, |l| l < 128)
    }

    pub fn is_light(&self) -> bool {
        parse_hex_luminance(&self.bg).map_or(false, |l| l >= 128)
    }

    pub fn merge(&self, other: &ColorTheme) -> ColorTheme {
        ColorTheme {
            name: format!("{}+{}", self.name, other.name),
            fg: other.fg.clone(),
            bg: self.bg.clone(),
            accent: other.accent.clone(),
        }
    }
}

fn parse_hex_luminance(hex: &str) -> Option<u8> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u32::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u32::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u32::from_str_radix(&hex[4..6], 16).ok()?;
    Some(((r * 299 + g * 587 + b * 114) / 1000) as u8)
}

// ---------------------------------------------------------------------------
// ThemeManager extensions
// ---------------------------------------------------------------------------

impl ThemeManager {
    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&ColorTheme> {
        self.themes.iter().find(|t| t.name == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ColorTheme> {
        self.themes.iter()
    }
}

// ---------------------------------------------------------------------------
// LayoutConstraint extensions
// ---------------------------------------------------------------------------

impl LayoutConstraint {
    pub fn is_fixed(&self) -> bool {
        matches!(self, LayoutConstraint::Fixed(_))
    }

    pub fn is_flexible(&self) -> bool {
        !self.is_fixed()
    }

    pub fn effective_size(&self, total: u16) -> u16 {
        match self {
            LayoutConstraint::Fixed(v) => *v,
            LayoutConstraint::Percentage(p) => ((total as f32) * p / 100.0) as u16,
            LayoutConstraint::Min(v) => *v,
            LayoutConstraint::Max(v) => (*v).min(total),
        }
    }
}

// ---------------------------------------------------------------------------
// RenderStats extensions
// ---------------------------------------------------------------------------

impl RenderStats {
    pub fn merge(&self, other: &RenderStats) -> RenderStats {
        RenderStats {
            frame_number: self.frame_number.max(other.frame_number),
            render_time_us: self.render_time_us + other.render_time_us,
            widget_count: self.widget_count + other.widget_count,
        }
    }
}

// ---------------------------------------------------------------------------
// RenderTracker extensions
// ---------------------------------------------------------------------------

impl RenderTracker {
    pub fn reset(&mut self) {
        self.frames.clear();
    }

    pub fn total_render_time_us(&self) -> u64 {
        self.frames.iter().map(|f| f.render_time_us).sum()
    }
}

// ---------------------------------------------------------------------------
// RenderRegion extensions
// ---------------------------------------------------------------------------

impl RenderRegion {
    pub fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    pub fn intersection(&self, other: &RenderRegion) -> Option<RenderRegion> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        if x1 < x2 && y1 < y2 {
            Some(RenderRegion::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// StatusBarItem extensions
// ---------------------------------------------------------------------------

impl StatusBarItem {
    pub fn is_visible(&self) -> bool {
        !self.text.is_empty()
    }

    pub fn has_tooltip(&self) -> bool {
        self.tooltip.is_some()
    }
}

// ---------------------------------------------------------------------------
// StatusBar iterator
// ---------------------------------------------------------------------------

impl StatusBar {
    pub fn iter(&self) -> impl Iterator<Item = &StatusBarItem> {
        self.items.iter()
    }
}

// ---------------------------------------------------------------------------
// Text wrapping
// ---------------------------------------------------------------------------

/// Word-wrap a string to fit within `width` columns.
///
/// Splits on whitespace boundaries when possible, falling back to hard breaks
/// for tokens longer than `width`.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![];
    }
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in &words {
            if word.len() > width {
                // Flush current line first.
                if !current.is_empty() {
                    lines.push(current);
                    current = String::new();
                }
                // Hard-break the long word.
                let mut remaining = *word;
                while remaining.len() > width {
                    lines.push(remaining[..width].to_string());
                    remaining = &remaining[width..];
                }
                current = remaining.to_string();
            } else if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        lines.push(current);
    }
    lines
}

// ---------------------------------------------------------------------------
// ANSI escape code stripping
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences (CSI, OSC, simple escapes) from a string.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next(); // consume '['
                    // Consume until a letter in '@'..='~' terminates the CSI.
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii_alphabetic() || c == '~' || c == '@' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next(); // consume ']'
                    // OSC — terminated by ST (\x1b\\) or BEL (\x07).
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {
                    // Simple two-byte escape — skip next char.
                    chars.next();
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Return the visible (non-ANSI) length of a string.
pub fn visible_len(s: &str) -> usize {
    strip_ansi(s).len()
}

// ---------------------------------------------------------------------------
// Text alignment
// ---------------------------------------------------------------------------

/// Horizontal text alignment within a fixed-width area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

/// Align `text` within `width` columns, padding with spaces.
///
/// If the text is longer than `width` it is truncated from the right.
pub fn align_text(text: &str, width: usize, alignment: TextAlignment) -> String {
    let text_len = text.len();
    if text_len >= width {
        return text[..width].to_string();
    }
    let padding = width - text_len;
    match alignment {
        TextAlignment::Left => format!("{}{}", text, " ".repeat(padding)),
        TextAlignment::Right => format!("{}{}", " ".repeat(padding), text),
        TextAlignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
        }
    }
}

// ---------------------------------------------------------------------------
// Border drawing characters
// ---------------------------------------------------------------------------

/// Box-drawing character sets for border rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Single-line borders: ┌─┐│└┘
    Single,
    /// Double-line borders: ╔═╗║╚╝
    Double,
    /// Rounded corners: ╭─╮│╰╯
    Rounded,
    /// ASCII-only borders: +-+|+-+
    Ascii,
}

/// The six characters needed to draw a rectangular border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

impl BorderStyle {
    /// Return the [`BorderChars`] for this style.
    pub fn chars(self) -> BorderChars {
        match self {
            BorderStyle::Single => BorderChars {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
            },
            BorderStyle::Double => BorderChars {
                top_left: '╔',
                top_right: '╗',
                bottom_left: '╚',
                bottom_right: '╝',
                horizontal: '═',
                vertical: '║',
            },
            BorderStyle::Rounded => BorderChars {
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                horizontal: '─',
                vertical: '│',
            },
            BorderStyle::Ascii => BorderChars {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
            },
        }
    }

    /// Render a complete top border line of `width` characters.
    pub fn top_line(self, width: usize) -> String {
        let c = self.chars();
        if width < 2 {
            return String::new();
        }
        let inner: String = std::iter::repeat(c.horizontal).take(width - 2).collect();
        format!("{}{}{}", c.top_left, inner, c.top_right)
    }

    /// Render a complete bottom border line of `width` characters.
    pub fn bottom_line(self, width: usize) -> String {
        let c = self.chars();
        if width < 2 {
            return String::new();
        }
        let inner: String = std::iter::repeat(c.horizontal).take(width - 2).collect();
        format!("{}{}{}", c.bottom_left, inner, c.bottom_right)
    }
}

// ---------------------------------------------------------------------------
// Scrollbar position calculation
// ---------------------------------------------------------------------------

/// Compute the scrollbar thumb position and size for a scrollable region.
///
/// Returns `(thumb_offset, thumb_size)` within `track_height` rows.
/// If the content fits entirely in the viewport the thumb spans the full
/// track.
pub fn scrollbar_metrics(
    content_height: usize,
    viewport_height: usize,
    scroll_offset: usize,
    track_height: usize,
) -> (usize, usize) {
    if content_height == 0 || viewport_height == 0 || track_height == 0 {
        return (0, track_height);
    }
    if content_height <= viewport_height {
        return (0, track_height);
    }
    // Thumb size proportional to visible fraction, at least 1 row.
    let thumb = (track_height * viewport_height / content_height).max(1).min(track_height);
    let max_offset = content_height - viewport_height;
    let scroll_frac = scroll_offset.min(max_offset) as f64 / max_offset as f64;
    let pos = ((track_height - thumb) as f64 * scroll_frac) as usize;
    (pos, thumb)
}

// ---------------------------------------------------------------------------
// CellGrid — 2-D character grid operations
// ---------------------------------------------------------------------------

/// A simple 2-D grid of characters for compositing terminal output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGrid {
    pub width: usize,
    pub height: usize,
    cells: Vec<char>,
}

impl CellGrid {
    /// Create a grid filled with spaces.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![' '; width * height],
        }
    }

    /// Get the character at `(x, y)`.
    pub fn get(&self, x: usize, y: usize) -> Option<char> {
        if x < self.width && y < self.height {
            Some(self.cells[y * self.width + x])
        } else {
            None
        }
    }

    /// Set the character at `(x, y)`.
    pub fn set(&mut self, x: usize, y: usize, ch: char) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x] = ch;
        }
    }

    /// Fill the entire grid with `ch`.
    pub fn fill(&mut self, ch: char) {
        self.cells.fill(ch);
    }

    /// Fill a rectangular sub-region with `ch`.
    pub fn fill_region(&mut self, x: usize, y: usize, w: usize, h: usize, ch: char) {
        for row in y..((y + h).min(self.height)) {
            for col in x..((x + w).min(self.width)) {
                self.cells[row * self.width + col] = ch;
            }
        }
    }

    /// Write a string starting at `(x, y)`, clipping at the grid edge.
    pub fn put_str(&mut self, x: usize, y: usize, s: &str) {
        if y >= self.height {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            let col = x + i;
            if col >= self.width {
                break;
            }
            self.cells[y * self.width + col] = ch;
        }
    }

    /// Return a row as a [`String`].
    pub fn row_str(&self, y: usize) -> Option<String> {
        if y >= self.height {
            return None;
        }
        let start = y * self.width;
        Some(self.cells[start..start + self.width].iter().collect())
    }

    /// Copy a rectangular region from `src` into this grid at `(dest_x, dest_y)`.
    pub fn blit(&mut self, src: &CellGrid, dest_x: usize, dest_y: usize) {
        for sy in 0..src.height {
            let dy = dest_y + sy;
            if dy >= self.height {
                break;
            }
            for sx in 0..src.width {
                let dx = dest_x + sx;
                if dx >= self.width {
                    break;
                }
                self.cells[dy * self.width + dx] = src.cells[sy * src.width + sx];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// TuiMouseRegion
// ---------------------------------------------------------------------------

pub struct TuiMouseRegion {
    pub id: String,
    pub x: u16, pub y: u16, pub width: u16, pub height: u16,
}

impl TuiMouseRegion {
    pub fn new(id: impl Into<String>, x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { id: id.into(), x, y, width, height }
    }
    pub fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
    pub fn area(&self) -> u32 { self.width as u32 * self.height as u32 }
}

impl fmt::Display for TuiMouseRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MouseRegion({}: {}x{} at {},{})", self.id, self.width, self.height, self.x, self.y)
    }
}

pub struct TuiMouseRegionRegistry { regions: Vec<TuiMouseRegion> }

impl TuiMouseRegionRegistry {
    pub fn new() -> Self { Self { regions: Vec::new() } }
    pub fn register(&mut self, region: TuiMouseRegion) { self.regions.push(region); }
    pub fn hit_test(&self, x: u16, y: u16) -> Option<&TuiMouseRegion> {
        self.regions.iter().rev().find(|r| r.contains(x, y))
    }
    pub fn clear(&mut self) { self.regions.clear(); }
    pub fn len(&self) -> usize { self.regions.len() }
    pub fn is_empty(&self) -> bool { self.regions.is_empty() }
}

impl Default for TuiMouseRegionRegistry { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// TuiDoubleClick
// ---------------------------------------------------------------------------

pub struct TuiDoubleClick {
    last_click_time_ms: Option<u64>,
    last_click_pos: Option<(u16, u16)>,
    threshold_ms: u64,
}

impl TuiDoubleClick {
    pub fn new(threshold_ms: u64) -> Self {
        Self { last_click_time_ms: None, last_click_pos: None, threshold_ms }
    }

    pub fn process_click(&mut self, x: u16, y: u16, time_ms: u64) -> bool {
        let is_double = match (self.last_click_time_ms, self.last_click_pos) {
            (Some(lt), Some((lx, ly))) => time_ms - lt <= self.threshold_ms && lx == x && ly == y,
            _ => false,
        };
        if is_double { self.last_click_time_ms = None; self.last_click_pos = None; }
        else { self.last_click_time_ms = Some(time_ms); self.last_click_pos = Some((x, y)); }
        is_double
    }

    pub fn reset(&mut self) { self.last_click_time_ms = None; self.last_click_pos = None; }
}

impl Default for TuiDoubleClick { fn default() -> Self { Self::new(500) } }

// ---------------------------------------------------------------------------
// TuiFocusTrap
// ---------------------------------------------------------------------------

pub struct TuiFocusTrap {
    active: bool,
    trapped_region: Option<TuiMouseRegion>,
    previous_focus_id: Option<String>,
}

impl TuiFocusTrap {
    pub fn new() -> Self { Self { active: false, trapped_region: None, previous_focus_id: None } }

    pub fn activate(&mut self, region: TuiMouseRegion, current_focus: Option<String>) {
        self.active = true; self.trapped_region = Some(region); self.previous_focus_id = current_focus;
    }

    pub fn deactivate(&mut self) -> Option<String> {
        self.active = false; self.trapped_region = None; self.previous_focus_id.take()
    }

    pub fn is_active(&self) -> bool { self.active }

    pub fn is_within_trap(&self, x: u16, y: u16) -> bool {
        self.trapped_region.as_ref().map_or(false, |r| r.contains(x, y))
    }
}

impl Default for TuiFocusTrap { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// TuiFrameRateController
// ---------------------------------------------------------------------------

pub struct TuiFrameRateController {
    target_fps: u32,
    frame_count: u64,
    last_frame_time_ms: u64,
}

impl TuiFrameRateController {
    pub fn new(target_fps: u32) -> Self {
        Self { target_fps, frame_count: 0, last_frame_time_ms: 0 }
    }
    pub fn target_fps(&self) -> u32 { self.target_fps }
    pub fn frame_duration_ms(&self) -> u64 { if self.target_fps == 0 { 0 } else { 1000 / self.target_fps as u64 } }
    pub fn should_render(&self, current_time_ms: u64) -> bool {
        current_time_ms >= self.last_frame_time_ms + self.frame_duration_ms()
    }
    pub fn record_frame(&mut self, time_ms: u64) { self.frame_count += 1; self.last_frame_time_ms = time_ms; }
    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn set_target_fps(&mut self, fps: u32) { self.target_fps = fps; }
}

impl Default for TuiFrameRateController { fn default() -> Self { Self::new(60) } }

impl fmt::Display for TuiFrameRateController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FrameRateController({}fps, {} frames)", self.target_fps, self.frame_count)
    }
}


// ---------------------------------------------------------------------------
// TuiMouseHoverTracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TuiMouseHoverTracker {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl TuiMouseHoverTracker {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for TuiMouseHoverTracker {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for TuiMouseHoverTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "TuiMouseHoverTracker({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// TuiClipboardIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TuiClipboardIntegration {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl TuiClipboardIntegration {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for TuiClipboardIntegration {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for TuiClipboardIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "TuiClipboardIntegration({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// TuiMouseHoverTrackerSnapshot — point-in-time snapshot of TuiMouseHoverTracker state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TuiMouseHoverTrackerSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl TuiMouseHoverTrackerSnapshot {
    pub fn capture(source: &TuiMouseHoverTracker, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for TuiMouseHoverTrackerSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// TuiClipboardIntegrationStats — aggregate statistics for TuiClipboardIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TuiClipboardIntegrationStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl TuiClipboardIntegrationStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for TuiClipboardIntegrationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// TuiMouseHoverTrackerConfig — configuration for TuiMouseHoverTracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TuiMouseHoverTrackerConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl TuiMouseHoverTrackerConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for TuiMouseHoverTrackerConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for TuiMouseHoverTrackerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}


// ─── TuiBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for UI events.
#[derive(Debug, Clone)]
pub struct TuiBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> TuiBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for TuiBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TuiBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── TuiFmt Formatter ───────────────────────────────────────

/// Formatting options for TUI output.
#[derive(Debug, Clone)]
pub struct TuiFmtFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for TuiFmtFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl TuiFmtFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for TUI data.
pub struct TuiFmtFmt {
    options: TuiFmtFmtOpts,
}

impl TuiFmtFmt {
    pub fn new(options: TuiFmtFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: TuiFmtFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Terminal ui configuration manager.
#[derive(Debug, Clone)]
pub struct TuiConfig {
    entries: Vec<TuiEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single terminal UI entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TuiEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl TuiEntry {
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

impl TuiConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: TuiEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&TuiEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut TuiEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&TuiEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&TuiEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&TuiEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<TuiEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_event_variants() {
        // Ensure all variants are constructible.
        let _tick = AppEvent::Tick;
        let _quit = AppEvent::Quit;
        let _resize = AppEvent::Resize(80, 24);
        let _paste = AppEvent::Paste("hello".into());
    }

    #[test]
    fn app_event_is_clone_and_debug() {
        let event = AppEvent::Tick;
        let _cloned = event.clone();
        let _dbg = format!("{event:?}");
    }

    /// Integration smoke test: create an App and immediately tear it down.
    ///
    /// Skipped when stdout is not a TTY (e.g. CI pipelines, piped output).
    #[test]
    fn setup_and_teardown() {
        if !atty_stdout() {
            eprintln!("skipping setup_and_teardown: stdout is not a TTY");
            return;
        }

        let mut terminal = setup_terminal().expect("setup_terminal should succeed");
        restore_terminal(&mut terminal).expect("restore_terminal should succeed");
    }

    /// Returns `true` when stdout is connected to a terminal.
    fn atty_stdout() -> bool {
        use std::os::fd::AsRawFd;
        let fd = io::stdout().as_raw_fd();
        // SAFETY: isatty is safe to call with any fd.
        unsafe { libc_free_isatty(fd) }
    }

    /// Minimal isatty without linking libc — reads `/proc/self/fd/<fd>` on
    /// Linux, falls back to assuming *not* a TTY elsewhere.
    #[cfg(target_os = "linux")]
    unsafe fn libc_free_isatty(fd: i32) -> bool {
        // /dev/pts/* or /dev/tty* indicates a terminal
        let link = format!("/proc/self/fd/{fd}");
        if let Ok(target) = std::fs::read_link(&link) {
            let s = target.to_string_lossy();
            return s.starts_with("/dev/pts/") || s.starts_with("/dev/tty");
        }
        false
    }

    #[cfg(not(target_os = "linux"))]
    unsafe fn libc_free_isatty(_fd: i32) -> bool {
        false
    }

    #[test]
    fn status_bar_item_creation() {
        let item = StatusBarItem::new("mode", "NORMAL")
            .with_alignment(StatusBarAlignment::Left)
            .with_priority(10)
            .with_tooltip("Current editor mode");
        assert_eq!(item.id, "mode");
        assert_eq!(item.text, "NORMAL");
        assert_eq!(item.alignment, StatusBarAlignment::Left);
        assert_eq!(item.priority, 10);
        assert_eq!(item.tooltip.as_deref(), Some("Current editor mode"));
    }

    #[test]
    fn status_bar_add_remove() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusBarItem::new("a", "Item A"));
        bar.add_item(StatusBarItem::new("b", "Item B"));
        assert_eq!(bar.item_count(), 2);
        assert!(bar.remove_item("a"));
        assert!(!bar.remove_item("a"));
        assert_eq!(bar.item_count(), 1);
    }

    #[test]
    fn status_bar_update_text() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusBarItem::new("mode", "NORMAL"));
        assert!(bar.update_text("mode", "INSERT"));
        assert_eq!(bar.get_item("mode").unwrap().text, "INSERT");
        assert!(!bar.update_text("missing", "X"));
    }

    #[test]
    fn status_bar_left_right_items() {
        let mut bar = StatusBar::new();
        bar.add_item(
            StatusBarItem::new("a", "A")
                .with_alignment(StatusBarAlignment::Left)
                .with_priority(5),
        );
        bar.add_item(
            StatusBarItem::new("b", "B")
                .with_alignment(StatusBarAlignment::Right)
                .with_priority(10),
        );
        bar.add_item(
            StatusBarItem::new("c", "C")
                .with_alignment(StatusBarAlignment::Left)
                .with_priority(10),
        );
        assert_eq!(bar.left_items().len(), 2);
        assert_eq!(bar.right_items().len(), 1);
        // Higher priority first
        assert_eq!(bar.left_items()[0].id, "c");
    }

    #[test]
    fn status_bar_clear() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusBarItem::new("a", "X"));
        bar.clear();
        assert_eq!(bar.item_count(), 0);
    }

    #[test]
    fn status_bar_render_text() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusBarItem::new("l", "LEFT").with_alignment(StatusBarAlignment::Left));
        bar.add_item(StatusBarItem::new("r", "RIGHT").with_alignment(StatusBarAlignment::Right));
        let rendered = bar.render_text(40);
        assert!(rendered.contains("LEFT"));
        assert!(rendered.contains("RIGHT"));
        assert_eq!(rendered.len(), 40);
    }

    #[test]
    fn status_bar_item_display() {
        let item = StatusBarItem::new("x", "Hello");
        assert_eq!(format!("{item}"), "Hello");
    }

    #[test]
    fn key_chord_parse_simple() {
        let chord = KeyChord::parse("S").unwrap();
        assert!(!chord.ctrl);
        assert!(!chord.alt);
        assert!(!chord.shift);
        assert_eq!(chord.key, "S");
    }

    #[test]
    fn key_chord_parse_modifiers() {
        let chord = KeyChord::parse("Ctrl+Shift+S").unwrap();
        assert!(chord.ctrl);
        assert!(!chord.alt);
        assert!(chord.shift);
        assert_eq!(chord.key, "S");
    }

    #[test]
    fn key_chord_parse_all_modifiers() {
        let chord = KeyChord::parse("Ctrl+Alt+Shift+F5").unwrap();
        assert!(chord.ctrl);
        assert!(chord.alt);
        assert!(chord.shift);
        assert_eq!(chord.key, "F5");
    }

    #[test]
    fn key_chord_parse_empty_returns_none() {
        assert!(KeyChord::parse("").is_none());
    }

    #[test]
    fn key_chord_display() {
        let chord = KeyChord {
            ctrl: true,
            alt: false,
            shift: true,
            key: "S".to_string(),
        };
        assert_eq!(format!("{chord}"), "Ctrl+Shift+S");
    }

    #[test]
    fn key_chord_matches_key_event_basic() {
        let chord = KeyChord::parse("Ctrl+S").unwrap();
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert!(chord.matches_key_event(&event));
    }

    #[test]
    fn key_chord_no_match_wrong_modifier() {
        let chord = KeyChord::parse("Ctrl+S").unwrap();
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT);
        assert!(!chord.matches_key_event(&event));
    }

    #[test]
    fn key_chord_matches_f_key() {
        let chord = KeyChord::parse("F5").unwrap();
        let event = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        assert!(chord.matches_key_event(&event));
    }

    #[test]
    fn key_chord_matches_enter() {
        let chord = KeyChord::parse("Enter").unwrap();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(chord.matches_key_event(&event));
    }

    #[test]
    fn key_chord_matches_escape() {
        let chord = KeyChord::parse("Escape").unwrap();
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(chord.matches_key_event(&event));
    }

    #[test]
    fn terminal_capabilities_default() {
        let caps = TerminalCapabilities::default();
        assert!(caps.supports_color);
        assert!(caps.supports_256_color);
        assert!(!caps.supports_true_color);
        assert!(caps.supports_mouse);
        assert!(caps.supports_bracketed_paste);
    }

    #[test]
    fn theme_manager_add_and_activate() {
        let mut tm = ThemeManager::new();
        tm.add_theme(ColorTheme {
            name: "dark".into(),
            fg: "#ccc".into(),
            bg: "#1e1e1e".into(),
            accent: "#007acc".into(),
        });
        assert!(tm.set_active("dark"));
        assert_eq!(tm.active_theme().unwrap().name, "dark");
        assert!(!tm.set_active("nonexistent"));
    }

    #[test]
    fn theme_manager_names() {
        let mut tm = ThemeManager::new();
        tm.add_theme(ColorTheme {
            name: "dark".into(),
            fg: "#ccc".into(),
            bg: "#111".into(),
            accent: "#0af".into(),
        });
        tm.add_theme(ColorTheme {
            name: "light".into(),
            fg: "#333".into(),
            bg: "#fff".into(),
            accent: "#00a".into(),
        });
        assert_eq!(tm.theme_names(), vec!["dark", "light"]);
    }

    #[test]
    fn resolve_constraints_basic() {
        let constraints = vec![
            LayoutConstraint::Fixed(10),
            LayoutConstraint::Percentage(50.0),
            LayoutConstraint::Min(5),
            LayoutConstraint::Max(100),
        ];
        let sizes = resolve_constraints(&constraints, 80);
        assert_eq!(sizes[0], 10);
        assert_eq!(sizes[1], 40);
        assert_eq!(sizes[2], 5);
        assert_eq!(sizes[3], 80);
    }

    #[test]
    fn render_tracker_stats() {
        let mut tracker = RenderTracker::new();
        tracker.record(RenderStats { frame_number: 1, render_time_us: 100, widget_count: 5 });
        tracker.record(RenderStats { frame_number: 2, render_time_us: 200, widget_count: 6 });
        tracker.record(RenderStats { frame_number: 3, render_time_us: 300, widget_count: 4 });
        assert_eq!(tracker.frame_count(), 3);
        assert_eq!(tracker.average_render_time_us(), 200);
        assert_eq!(tracker.last_frame().unwrap().frame_number, 3);
    }

    #[test]
    fn render_tracker_empty() {
        let tracker = RenderTracker::new();
        assert_eq!(tracker.frame_count(), 0);
        assert_eq!(tracker.average_render_time_us(), 0);
        assert!(tracker.last_frame().is_none());
    }

    // -- TerminalCapabilities extended methods --

    #[test]
    fn capabilities_sixel_placeholder() {
        let caps = TerminalCapabilities::default();
        assert!(!caps.supports_sixel());
    }

    #[test]
    fn capabilities_kitty_graphics_placeholder() {
        let caps = TerminalCapabilities::default();
        assert!(!caps.supports_kitty_graphics());
    }

    #[test]
    fn capabilities_summary_contains_fields() {
        let caps = TerminalCapabilities::default();
        let summary = caps.capability_summary();
        assert!(summary.contains("truecolor:"));
        assert!(summary.contains("mouse:"));
        assert!(summary.contains("bracketed_paste:"));
        assert!(summary.contains("sixel:"));
        assert!(summary.contains("kitty_graphics:"));
    }

    #[test]
    fn capabilities_from_env() {
        // from_env should return a valid struct regardless of env state
        let caps = TerminalCapabilities::from_env();
        // mouse and bracketed_paste are always true
        assert!(caps.supports_mouse);
        assert!(caps.supports_bracketed_paste);
    }

    // -- terminal_size --

    #[test]
    fn terminal_size_returns_nonzero() {
        let (cols, rows) = terminal_size();
        assert!(cols > 0);
        assert!(rows > 0);
    }

    // -- RenderRegion --

    #[test]
    fn render_region_new_defaults() {
        let r = RenderRegion::new(5, 10, 40, 3);
        assert_eq!(r.x, 5);
        assert_eq!(r.y, 10);
        assert_eq!(r.width, 40);
        assert_eq!(r.height, 3);
        assert!(r.is_dirty());
        assert_eq!(r.content.len(), 3);
    }

    #[test]
    fn render_region_set_and_get_line() {
        let mut r = RenderRegion::new(0, 0, 80, 4);
        r.mark_clean();
        assert!(!r.is_dirty());
        r.set_line(2, "hello".into());
        assert!(r.is_dirty());
        assert_eq!(r.get_line(2), Some("hello"));
        assert_eq!(r.get_line(0), Some(""));
        assert_eq!(r.get_line(99), None);
    }

    #[test]
    fn render_region_clear() {
        let mut r = RenderRegion::new(0, 0, 10, 2);
        r.set_line(0, "data".into());
        r.mark_clean();
        r.clear();
        assert!(r.is_dirty());
        assert_eq!(r.get_line(0), Some(""));
    }

    #[test]
    fn render_region_dirty_tracking() {
        let mut r = RenderRegion::new(0, 0, 10, 2);
        assert!(r.is_dirty()); // new region is dirty
        r.mark_clean();
        assert!(!r.is_dirty());
        r.mark_dirty();
        assert!(r.is_dirty());
    }

    #[test]
    fn render_region_overlaps() {
        let a = RenderRegion::new(0, 0, 10, 10);
        let b = RenderRegion::new(5, 5, 10, 10);
        let c = RenderRegion::new(20, 20, 5, 5);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
        assert!(!c.overlaps(&a));
    }

    #[test]
    fn render_region_contains_point() {
        let r = RenderRegion::new(10, 20, 5, 3);
        assert!(r.contains_point(10, 20));
        assert!(r.contains_point(14, 22));
        assert!(!r.contains_point(15, 20)); // x == x+width is out
        assert!(!r.contains_point(10, 23)); // y == y+height is out
        assert!(!r.contains_point(9, 20));
    }

    // -- KeyChord extensions --

    #[test]
    fn key_chord_modifier_count() {
        let chord = KeyChord::parse("Ctrl+Alt+Shift+X").unwrap();
        assert_eq!(chord.modifier_count(), 3);
        assert!(chord.has_modifiers());

        let plain = KeyChord::parse("A").unwrap();
        assert_eq!(plain.modifier_count(), 0);
        assert!(!plain.has_modifiers());
    }

    // -- ColorTheme extensions --

    #[test]
    fn color_theme_dark_light() {
        let dark = ColorTheme {
            name: "dark".into(),
            fg: "#cccccc".into(),
            bg: "#1e1e1e".into(),
            accent: "#007acc".into(),
        };
        assert!(dark.is_dark());
        assert!(!dark.is_light());

        let light = ColorTheme {
            name: "light".into(),
            fg: "#333333".into(),
            bg: "#ffffff".into(),
            accent: "#0000aa".into(),
        };
        assert!(light.is_light());
        assert!(!light.is_dark());
    }

    #[test]
    fn color_theme_has_color_and_merge() {
        let a = ColorTheme {
            name: "a".into(),
            fg: "#aaa".into(),
            bg: "#111".into(),
            accent: "#f00".into(),
        };
        let b = ColorTheme {
            name: "b".into(),
            fg: "#bbb".into(),
            bg: "#222".into(),
            accent: "#0f0".into(),
        };
        assert!(a.has_color("#f00"));
        assert!(!a.has_color("#999"));
        assert_eq!(a.color_count(), 3);

        let merged = a.merge(&b);
        assert_eq!(merged.name, "a+b");
        assert_eq!(merged.fg, "#bbb");
        assert_eq!(merged.bg, "#111");
        assert_eq!(merged.accent, "#0f0");
    }

    // -- ThemeManager extensions --

    #[test]
    fn theme_manager_find_and_iter() {
        let mut tm = ThemeManager::new();
        tm.add_theme(ColorTheme {
            name: "dark".into(),
            fg: "#ccc".into(),
            bg: "#111".into(),
            accent: "#0af".into(),
        });
        tm.add_theme(ColorTheme {
            name: "light".into(),
            fg: "#333".into(),
            bg: "#fff".into(),
            accent: "#00a".into(),
        });
        assert_eq!(tm.theme_count(), 2);
        assert_eq!(tm.find_by_name("dark").unwrap().name, "dark");
        assert!(tm.find_by_name("missing").is_none());
        assert_eq!(tm.iter().count(), 2);
    }

    // -- LayoutConstraint extensions --

    #[test]
    fn layout_constraint_is_fixed_flexible() {
        assert!(LayoutConstraint::Fixed(10).is_fixed());
        assert!(!LayoutConstraint::Fixed(10).is_flexible());
        assert!(LayoutConstraint::Percentage(50.0).is_flexible());
        assert!(LayoutConstraint::Min(5).is_flexible());
        assert!(LayoutConstraint::Max(100).is_flexible());
        assert_eq!(LayoutConstraint::Percentage(25.0).effective_size(200), 50);
        assert_eq!(LayoutConstraint::Fixed(42).effective_size(200), 42);
    }

    // -- RenderStats merge --

    #[test]
    fn render_stats_merge() {
        let a = RenderStats { frame_number: 1, render_time_us: 100, widget_count: 5 };
        let b = RenderStats { frame_number: 3, render_time_us: 200, widget_count: 7 };
        let merged = a.merge(&b);
        assert_eq!(merged.frame_number, 3);
        assert_eq!(merged.render_time_us, 300);
        assert_eq!(merged.widget_count, 12);
    }

    // -- RenderTracker reset --

    #[test]
    fn render_tracker_reset_and_total() {
        let mut tracker = RenderTracker::new();
        tracker.record(RenderStats { frame_number: 1, render_time_us: 100, widget_count: 5 });
        tracker.record(RenderStats { frame_number: 2, render_time_us: 200, widget_count: 6 });
        assert_eq!(tracker.total_render_time_us(), 300);
        tracker.reset();
        assert_eq!(tracker.frame_count(), 0);
        assert_eq!(tracker.total_render_time_us(), 0);
    }

    // -- RenderRegion area and intersection --

    #[test]
    fn render_region_area_and_intersection() {
        let a = RenderRegion::new(0, 0, 10, 10);
        assert_eq!(a.area(), 100);

        let b = RenderRegion::new(5, 5, 10, 10);
        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter.x, 5);
        assert_eq!(inter.y, 5);
        assert_eq!(inter.width, 5);
        assert_eq!(inter.height, 5);
        assert_eq!(inter.area(), 25);

        let c = RenderRegion::new(20, 20, 5, 5);
        assert!(a.intersection(&c).is_none());
    }

    // -- StatusBarItem extensions --

    #[test]
    fn status_bar_item_visibility_and_tooltip() {
        let visible = StatusBarItem::new("a", "text").with_tooltip("tip");
        assert!(visible.is_visible());
        assert!(visible.has_tooltip());

        let empty = StatusBarItem::new("b", "");
        assert!(!empty.is_visible());
        assert!(!empty.has_tooltip());
    }

    // -- StatusBar iterator --

    #[test]
    fn status_bar_iter() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusBarItem::new("a", "A"));
        bar.add_item(StatusBarItem::new("b", "B"));
        let ids: Vec<_> = bar.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    // -- TerminalCapabilities enabled_count --

    #[test]
    fn capabilities_enabled_count() {
        let caps = TerminalCapabilities::default();
        assert_eq!(caps.enabled_count(), 4);

        let all = TerminalCapabilities {
            supports_color: true,
            supports_256_color: true,
            supports_true_color: true,
            supports_mouse: true,
            supports_bracketed_paste: true,
        };
        assert_eq!(all.enabled_count(), 5);
    }

    // -- wrap_text --

    #[test]
    fn wrap_text_basic() {
        let lines = wrap_text("hello world foo bar", 11);
        assert_eq!(lines, vec!["hello world", "foo bar"]);
    }

    #[test]
    fn wrap_text_long_word_hard_break() {
        let lines = wrap_text("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_text_preserves_newlines() {
        let lines = wrap_text("a\nb\nc", 80);
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn wrap_text_zero_width() {
        assert!(wrap_text("hello", 0).is_empty());
    }

    // -- strip_ansi / visible_len --

    #[test]
    fn strip_ansi_removes_csi() {
        let input = "\x1b[31mred\x1b[0m plain";
        assert_eq!(strip_ansi(input), "red plain");
    }

    #[test]
    fn strip_ansi_no_escapes() {
        assert_eq!(strip_ansi("hello"), "hello");
    }

    #[test]
    fn visible_len_ansi() {
        assert_eq!(visible_len("\x1b[1mbold\x1b[0m"), 4);
    }

    // -- align_text --

    #[test]
    fn align_text_left() {
        assert_eq!(align_text("hi", 6, TextAlignment::Left), "hi    ");
    }

    #[test]
    fn align_text_right() {
        assert_eq!(align_text("hi", 6, TextAlignment::Right), "    hi");
    }

    #[test]
    fn align_text_center() {
        assert_eq!(align_text("hi", 6, TextAlignment::Center), "  hi  ");
    }

    #[test]
    fn align_text_truncate() {
        assert_eq!(align_text("longtext", 4, TextAlignment::Left), "long");
    }

    // -- BorderStyle --

    #[test]
    fn border_single_chars() {
        let c = BorderStyle::Single.chars();
        assert_eq!(c.top_left, '┌');
        assert_eq!(c.horizontal, '─');
    }

    #[test]
    fn border_top_line() {
        assert_eq!(BorderStyle::Ascii.top_line(5), "+---+");
        assert_eq!(BorderStyle::Ascii.bottom_line(5), "+---+");
    }

    #[test]
    fn border_top_line_too_narrow() {
        assert_eq!(BorderStyle::Single.top_line(1), "");
    }

    // -- scrollbar_metrics --

    #[test]
    fn scrollbar_fits_in_viewport() {
        let (pos, size) = scrollbar_metrics(10, 20, 0, 10);
        assert_eq!(pos, 0);
        assert_eq!(size, 10);
    }

    #[test]
    fn scrollbar_half_scrolled() {
        let (pos, size) = scrollbar_metrics(100, 10, 45, 20);
        assert!(size >= 1);
        assert!(pos + size <= 20);
    }

    // -- CellGrid --

    #[test]
    fn cell_grid_new_filled_with_spaces() {
        let g = CellGrid::new(4, 3);
        assert_eq!(g.get(0, 0), Some(' '));
        assert_eq!(g.row_str(0), Some("    ".to_string()));
    }

    #[test]
    fn cell_grid_set_get() {
        let mut g = CellGrid::new(5, 5);
        g.set(2, 3, 'X');
        assert_eq!(g.get(2, 3), Some('X'));
        assert_eq!(g.get(99, 0), None);
    }

    #[test]
    fn cell_grid_fill_region() {
        let mut g = CellGrid::new(6, 4);
        g.fill_region(1, 1, 3, 2, '#');
        assert_eq!(g.get(0, 0), Some(' '));
        assert_eq!(g.get(1, 1), Some('#'));
        assert_eq!(g.get(3, 2), Some('#'));
        assert_eq!(g.get(4, 1), Some(' '));
    }

    #[test]
    fn cell_grid_put_str_and_row() {
        let mut g = CellGrid::new(10, 2);
        g.put_str(2, 0, "hello");
        assert_eq!(g.row_str(0), Some("  hello   ".to_string()));
    }

    #[test]
    fn cell_grid_blit() {
        let mut dst = CellGrid::new(6, 4);
        let mut src = CellGrid::new(2, 2);
        src.fill('A');
        dst.blit(&src, 1, 1);
        assert_eq!(dst.get(0, 0), Some(' '));
        assert_eq!(dst.get(1, 1), Some('A'));
        assert_eq!(dst.get(2, 2), Some('A'));
        assert_eq!(dst.get(3, 1), Some(' '));
    }


    #[test]
    fn mouse_region_contains() {
        let r = TuiMouseRegion::new("btn", 10, 20, 5, 3);
        assert!(r.contains(10, 20));
        assert!(r.contains(14, 22));
        assert!(!r.contains(15, 20));
    }

    #[test]
    fn mouse_region_area() {
        assert_eq!(TuiMouseRegion::new("x", 0, 0, 10, 5).area(), 50);
    }

    #[test]
    fn mouse_registry_hit() {
        let mut reg = TuiMouseRegionRegistry::new();
        reg.register(TuiMouseRegion::new("a", 0, 0, 10, 10));
        reg.register(TuiMouseRegion::new("b", 5, 5, 10, 10));
        assert_eq!(reg.hit_test(7, 7).unwrap().id, "b");
    }

    #[test]
    fn mouse_registry_miss() {
        assert!(TuiMouseRegionRegistry::new().hit_test(0, 0).is_none());
    }

    #[test]
    fn double_click_detection() {
        let mut dc = TuiDoubleClick::new(300);
        assert!(!dc.process_click(5, 5, 100));
        assert!(dc.process_click(5, 5, 200));
    }

    #[test]
    fn double_click_too_slow() {
        let mut dc = TuiDoubleClick::new(300);
        dc.process_click(5, 5, 100);
        assert!(!dc.process_click(5, 5, 500));
    }

    #[test]
    fn double_click_diff_pos() {
        let mut dc = TuiDoubleClick::new(300);
        dc.process_click(5, 5, 100);
        assert!(!dc.process_click(6, 5, 200));
    }

    #[test]
    fn focus_trap_activate() {
        let mut trap = TuiFocusTrap::new();
        trap.activate(TuiMouseRegion::new("d", 10, 10, 20, 15), Some("ed".into()));
        assert!(trap.is_active());
        assert!(trap.is_within_trap(15, 15));
        assert!(!trap.is_within_trap(5, 5));
    }

    #[test]
    fn focus_trap_deactivate() {
        let mut trap = TuiFocusTrap::new();
        trap.activate(TuiMouseRegion::new("d", 0, 0, 10, 10), Some("prev".into()));
        assert_eq!(trap.deactivate(), Some("prev".into()));
        assert!(!trap.is_active());
    }

    #[test]
    fn frame_rate_basic() {
        let mut frc = TuiFrameRateController::new(60);
        assert_eq!(frc.frame_duration_ms(), 16);
        assert!(frc.should_render(16));
        frc.record_frame(16);
        assert!(!frc.should_render(26));
        assert!(frc.should_render(33));
    }

    #[test]
    fn frame_rate_display() {
        assert!(format!("{}", TuiFrameRateController::new(30)).contains("30fps"));
    }

    #[test]
    fn mouse_region_display() {
        assert!(format!("{}", TuiMouseRegion::new("btn", 0, 0, 10, 5)).contains("btn"));
    }


    #[test] fn tuiMouseHoverTracker_new() { let s = TuiMouseHoverTracker::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn tuiMouseHoverTracker_add() { let mut s = TuiMouseHoverTracker::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn tuiMouseHoverTracker_remove() { let mut s = TuiMouseHoverTracker::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn tuiMouseHoverTracker_config() { let mut s = TuiMouseHoverTracker::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn tuiMouseHoverTracker_nav() { let mut s = TuiMouseHoverTracker::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn tuiMouseHoverTracker_filter() { let mut s = TuiMouseHoverTracker::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn tuiMouseHoverTracker_display() { assert!(format!("{}", TuiMouseHoverTracker::new()).contains("TuiMouseHoverTracker")); }
    #[test] fn tuiClipboardIntegration_new() { let s = TuiClipboardIntegration::new(); assert!(s.is_empty()); }
    #[test] fn tuiClipboardIntegration_add() { let mut s = TuiClipboardIntegration::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn tuiClipboardIntegration_active() { let mut s = TuiClipboardIntegration::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn tuiClipboardIntegration_error() { let mut s = TuiClipboardIntegration::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn tuiClipboardIntegration_rm_group() { let mut s = TuiClipboardIntegration::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn tuiClipboardIntegration_display() { assert!(format!("{}", TuiClipboardIntegration::new()).contains("TuiClipboardIntegration")); }


    #[test] fn tuiMouseHoverTracker_snap_capture() {
        let s = TuiMouseHoverTracker::new();
        let snap = TuiMouseHoverTrackerSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn tuiMouseHoverTracker_snap_stale() {
        let s = TuiMouseHoverTracker::new();
        let snap = TuiMouseHoverTrackerSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn tuiMouseHoverTracker_snap_diff() {
        let s = TuiMouseHoverTracker::new();
        let s1v = TuiMouseHoverTrackerSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn tuiMouseHoverTracker_snap_display() {
        let s = TuiMouseHoverTracker::new();
        let snap = TuiMouseHoverTrackerSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn tuiClipboardIntegration_stats_record() {
        let mut st = TuiClipboardIntegrationStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn tuiClipboardIntegration_stats_hit_ratio() {
        let mut st = TuiClipboardIntegrationStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn tuiClipboardIntegration_stats_merge() {
        let mut a = TuiClipboardIntegrationStats::new();
        a.total_adds = 5;
        let mut b = TuiClipboardIntegrationStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn tuiClipboardIntegration_stats_display() {
        let st = TuiClipboardIntegrationStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn tuiMouseHoverTracker_config_default() {
        let c = TuiMouseHoverTrackerConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn tuiMouseHoverTracker_config_builder() {
        let c = TuiMouseHoverTrackerConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn tuiMouseHoverTracker_config_labels() {
        let mut c = TuiMouseHoverTrackerConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn tuiMouseHoverTracker_config_cleanup_threshold() {
        let c = TuiMouseHoverTrackerConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn tuiMouseHoverTracker_config_display() {
        assert!(format!("{}", TuiMouseHoverTrackerConfig::new()).contains("Config"));
    }
    #[test] fn tuiClipboardIntegration_stats_peaks() {
        let mut st = TuiClipboardIntegrationStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }


    #[test]
    fn tuibuf_ringbuf_push_get() {
        let mut rb = TuiBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn tuibuf_ringbuf_overflow() {
        let mut rb = TuiBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn tuibuf_ringbuf_clear() {
        let mut rb = TuiBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn tuibuf_ringbuf_newest_oldest() {
        let mut rb = TuiBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn tuibuf_ringbuf_to_vec() {
        let mut rb = TuiBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn tuibuf_ringbuf_is_full() {
        let mut rb = TuiBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn tuifmt_fmt_list() {
        let f = TuiFmtFmt::new(TuiFmtFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn tuifmt_fmt_kv() {
        let f = TuiFmtFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn tuifmt_fmt_section() {
        let f = TuiFmtFmt::new(TuiFmtFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn tuifmt_fmt_truncate() {
        let f = TuiFmtFmt::new(TuiFmtFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn tuifmt_fmt_opts_defaults() {
        let o = TuiFmtFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn tui_entry_creation() {
        let e = TuiEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn tui_entry_with_priority() {
        let e = TuiEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn tui_entry_metadata() {
        let e = TuiEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn tui_entry_remove_meta() {
        let mut e = TuiEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn tui_entry_activate_deactivate() {
        let mut e = TuiEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn tui_config_add_sorted() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("lo", "Lo").with_priority(1));
        c.add(TuiEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn tui_config_capacity() {
        let mut c = TuiConfig::new(1);
        assert!(c.add(TuiEntry::new("a", "A")));
        assert!(!c.add(TuiEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn tui_config_remove() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn tui_config_get() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn tui_config_active_entries() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        c.add(TuiEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn tui_config_enable_disable() {
        let mut c = TuiConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn tui_config_clear() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn tui_config_find_by_label() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn tui_config_top_n() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A").with_priority(1));
        c.add(TuiEntry::new("b", "B").with_priority(2));
        c.add(TuiEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn tui_config_deactivate_activate_all() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        c.add(TuiEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn tui_config_highest_priority() {
        let mut c = TuiConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(TuiEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn tui_config_contains() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn tui_config_labels() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "Alpha"));
        c.add(TuiEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn tui_config_drain_inactive() {
        let mut c = TuiConfig::new(10);
        c.add(TuiEntry::new("a", "A"));
        c.add(TuiEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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

}