//! vsedit main binary — terminal port of Visual Studio Code.
//!
//! Entry point that ties together the TUI framework, workbench, input handling,
//! and editor widget into a working terminal editor.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{Event as CtEvent, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use vsedit_editor_widget::EditorWidget;
use vsedit_input::{from_crossterm_key, from_crossterm_mouse};
use vsedit_tui::{restore_terminal, setup_terminal};

/// Editor state managed across the event loop.
struct EditorState {
    /// The workbench instance (used for lifecycle; rendering delegated in the future).
    _workbench: vsedit_workbench::Workbench,
    /// The editor widget.
    editor: EditorWidget,
    /// File path being edited, if any.
    file_path: Option<PathBuf>,
    /// Text content of the open file.
    content: String,
    /// Whether the application should quit.
    should_quit: bool,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Parse command-line arguments: optional file path.
    let file_path = std::env::args().nth(1).map(PathBuf::from);

    // Initialize logging.
    vsedit_log::init_tracing(vsedit_log::LogLevel::Info);
    tracing::info!("vsedit starting");

    // Load product configuration.
    let product = vsedit_product::ProductConfiguration::default_config();
    tracing::info!("{} v{}", product.name_long, product.version);

    // Set up environment and ensure data directories exist.
    let args = vsedit_environment::CliArgs {
        paths: file_path.iter().cloned().collect(),
        ..Default::default()
    };
    let env_svc = vsedit_environment::EnvironmentService::new(args);
    if let Err(e) = env_svc.paths.ensure_dirs() {
        tracing::warn!("Could not create data directories: {}", e);
    }

    // Initialize workbench.
    let mut workbench = vsedit_workbench::Workbench::new();
    workbench.start();

    // Set up editor widget and optionally load a file.
    let mut editor = EditorWidget::new();
    editor.is_focused = true;

    let content = match &file_path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => {
                tracing::info!("Opened: {}", path.display());
                text
            }
            Err(e) => {
                tracing::warn!("Could not read {}: {}", path.display(), e);
                String::new()
            }
        },
        None => String::new(),
    };

    let mut state = EditorState {
        _workbench: workbench,
        editor,
        file_path,
        content,
        should_quit: false,
    };

    // Set up terminal.
    let mut terminal = setup_terminal()?;

    // Run the event loop; always restore terminal on exit.
    let result = run_event_loop(&mut terminal, &mut state).await;

    restore_terminal(&mut terminal)?;
    tracing::info!("vsedit exiting");
    result
}

/// Main event loop: reads crossterm events, dispatches input, and renders.
async fn run_event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    state: &mut EditorState,
) -> io::Result<()> {
    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));

    loop {
        // Render a frame.
        terminal.draw(|frame| render(frame, state))?;

        if state.should_quit {
            break;
        }

        // Wait for the next terminal event or tick.
        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => handle_event(event, state),
                    Some(Err(_)) | None => {
                        state.should_quit = true;
                    }
                }
            }
            _ = tick_interval.tick() => {}
        }
    }

    Ok(())
}

/// Convert a crossterm event into our input model and handle it.
fn handle_event(event: CtEvent, state: &mut EditorState) {
    match event {
        CtEvent::Key(key_event) => {
            // Ctrl+Q or Ctrl+C quits.
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                match key_event.code {
                    KeyCode::Char('q') | KeyCode::Char('c') => {
                        state.should_quit = true;
                        return;
                    }
                    _ => {}
                }
            }

            let _input = from_crossterm_key(key_event);
            // Future: route through workbench.handle_input(_input)
        }
        CtEvent::Mouse(mouse_event) => {
            let _input = from_crossterm_mouse(mouse_event);
            // Future: route through workbench.handle_input(_input)
        }
        CtEvent::Resize(_cols, _rows) => {
            // Terminal will re-render on next frame automatically.
        }
        CtEvent::Paste(_text) => {
            // Future: insert pasted text into the editor buffer.
        }
        CtEvent::FocusGained | CtEvent::FocusLost => {}
    }
}

/// Render the full editor UI into the terminal frame.
fn render(frame: &mut ratatui::Frame, state: &EditorState) {
    let area = frame.area();

    // Layout: title bar (1 line) | editor area | status bar (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Title bar.
    let title_text = match &state.file_path {
        Some(path) => format!(" vsedit — {}", path.display()),
        None => " vsedit — [No File]".to_string(),
    };
    let title_bar = Paragraph::new(Line::from(vec![
        Span::styled(title_text, Style::default().fg(Color::Black).bg(Color::Cyan)),
    ]))
    .style(Style::default().bg(Color::Cyan));
    frame.render_widget(title_bar, chunks[0]);

    // Editor area.
    let editor_block = Block::default().borders(Borders::NONE);
    let lines: Vec<Line> = if state.content.is_empty() {
        vec![Line::from(Span::styled(
            "  (empty — open a file: vsedit <path>)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let show_line_numbers = state.editor.show_line_numbers;
        let num_width = state.content.lines().count().to_string().len();
        state
            .content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if show_line_numbers {
                    Line::from(vec![
                        Span::styled(
                            format!(" {:>width$} ", i + 1, width = num_width),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(line),
                    ])
                } else {
                    Line::from(Span::raw(format!(" {line}")))
                }
            })
            .collect()
    };
    let editor_paragraph = Paragraph::new(lines).block(editor_block);
    frame.render_widget(editor_paragraph, chunks[1]);

    // Status bar.
    let line_count = state.content.lines().count();
    let status_text = format!(
        " {} | {} lines | Ctrl+Q to quit",
        state
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[untitled]".to_string()),
        line_count,
    );
    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(status_text, Style::default().fg(Color::Black).bg(Color::Blue)),
    ]))
    .style(Style::default().bg(Color::Blue));
    frame.render_widget(status_bar, chunks[2]);
}
