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
// Tests
// ---------------------------------------------------------------------------

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
}
