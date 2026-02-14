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
}
