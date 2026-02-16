//! vsedit main binary — terminal port of Visual Studio Code.
//!
//! Entry point that ties together the TUI framework, workbench, input handling,
//! and editor controller into a working terminal editor.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{Event as CtEvent, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;

use vsedit_editor_controller::{EditorAction, EditorController};
use vsedit_input::{from_crossterm_key, InputEvent};
use vsedit_tui::{restore_terminal, setup_terminal};
use vsedit_workbench::{FocusedPart, WorkbenchAction, Workbench};

#[tokio::main]
async fn main() -> io::Result<()> {
    let file_path = std::env::args().nth(1).map(PathBuf::from);

    vsedit_log::init_tracing(vsedit_log::LogLevel::Info);
    tracing::info!("vsedit starting");

    let product = vsedit_product::ProductConfiguration::default_config();
    tracing::info!("{} v{}", product.name_long, product.version);

    let args = vsedit_environment::CliArgs {
        paths: file_path.iter().cloned().collect(),
        ..Default::default()
    };
    let env_svc = vsedit_environment::EnvironmentService::new(args);
    if let Err(e) = env_svc.paths.ensure_dirs() {
        tracing::warn!("Could not create data directories: {}", e);
    }

    // Load file content.
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

    let mut workbench = Workbench::new();
    workbench.start();

    let mut controller = EditorController::new(&content);

    // Sync initial state to workbench. Open the file as a tab.
    if let Some(ref path) = file_path {
        workbench.open_file(path, &content);
    } else {
        workbench.set_editor_content(&controller.model.get_value(), None);
    }
    let pos = controller.cursors.get_primary().position();
    workbench.set_cursor_info(pos.line, pos.column);

    let mut terminal = setup_terminal()?;

    let result = run_event_loop(
        &mut terminal,
        &mut workbench,
        &mut controller,
        &file_path,
        &path_str,
    )
    .await;

    restore_terminal(&mut terminal)?;
    tracing::info!("vsedit exiting");
    result
}

async fn run_event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    workbench: &mut Workbench,
    controller: &mut EditorController,
    file_path: &Option<PathBuf>,
    path_str: &Option<String>,
) -> io::Result<()> {
    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
    let mut should_quit = false;

    loop {
        terminal.draw(|frame| workbench.render(frame))?;

        if should_quit {
            break;
        }

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        should_quit = handle_event(
                            event, workbench, controller, file_path, path_str,
                        );
                    }
                    Some(Err(_)) | None => {
                        should_quit = true;
                    }
                }
            }
            _ = tick_interval.tick() => {}
        }
    }

    Ok(())
}

/// Returns true if the application should quit.
fn handle_event(
    event: CtEvent,
    workbench: &mut Workbench,
    controller: &mut EditorController,
    file_path: &Option<PathBuf>,
    path_str: &Option<String>,
) -> bool {
    match event {
        CtEvent::Key(key_event) => {
            let has_ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
            let has_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);

            // When command palette is open, route all keys through workbench.
            if workbench.focused == FocusedPart::CommandPalette {
                let input = from_crossterm_key(key_event);
                let action = workbench.handle_input(InputEvent::Key(input));
                match action {
                    WorkbenchAction::ExecuteCommand(ref cmd) => {
                        if cmd == "workbench.action.quit" {
                            return true;
                        }
                        if cmd == "workbench.action.files.save" {
                            if let Some(path) = file_path {
                                let value = controller.model.get_value();
                                if let Err(e) = std::fs::write(path, &value) {
                                    tracing::error!("Failed to save: {}", e);
                                } else {
                                    tracing::info!("Saved: {}", path.display());
                                    workbench.is_modified = false;
                                }
                            }
                            sync_state(workbench, controller, path_str);
                            return false;
                        }
                        workbench.execute_command(cmd);
                    }
                    _ => {}
                }
                return false;
            }

            // Ctrl+key combos: route through workbench or handle directly.
            if has_ctrl {
                match key_event.code {
                    KeyCode::Char('s') => {
                        if let Some(path) = file_path {
                            let value = controller.model.get_value();
                            if let Err(e) = std::fs::write(path, &value) {
                                tracing::error!("Failed to save: {}", e);
                            } else {
                                tracing::info!("Saved: {}", path.display());
                                workbench.is_modified = false;
                            }
                        }
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    KeyCode::Char('z') => {
                        controller.execute_action(EditorAction::Undo);
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    KeyCode::Char('y') => {
                        controller.execute_action(EditorAction::Redo);
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    KeyCode::Char('a') => {
                        controller.execute_action(EditorAction::SelectAll);
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    KeyCode::Home => {
                        controller.execute_action(EditorAction::MoveCursorDocumentStart);
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    KeyCode::End => {
                        controller.execute_action(EditorAction::MoveCursorDocumentEnd);
                        sync_state(workbench, controller, path_str);
                        return false;
                    }
                    _ => {
                        // Route through workbench keybinding resolver.
                        let input = from_crossterm_key(key_event);
                        let action = workbench.handle_input(InputEvent::Key(input));
                        match action {
                            WorkbenchAction::ExecuteCommand(ref cmd) => {
                                if cmd == "workbench.action.quit" {
                                    return true;
                                }
                                if cmd == "workbench.action.files.save" {
                                    if let Some(path) = file_path {
                                        let value = controller.model.get_value();
                                        if let Err(e) = std::fs::write(path, &value) {
                                            tracing::error!("Failed to save: {}", e);
                                        } else {
                                            tracing::info!("Saved: {}", path.display());
                                            workbench.is_modified = false;
                                        }
                                    }
                                    sync_state(workbench, controller, path_str);
                                    return false;
                                }
                                workbench.execute_command(cmd);
                            }
                            _ => {}
                        }
                        return false;
                    }
                }
            }

            // Non-ctrl key events → editor actions.
            let editor_action = match key_event.code {
                KeyCode::Char(c) => Some(EditorAction::InsertText(c.to_string())),
                KeyCode::Backspace => Some(EditorAction::DeleteLeft),
                KeyCode::Delete => Some(EditorAction::DeleteRight),
                KeyCode::Enter => Some(EditorAction::NewLine),
                KeyCode::Tab => Some(EditorAction::IndentLine),
                KeyCode::Left => {
                    if has_shift {
                        Some(EditorAction::SelectLeft)
                    } else {
                        Some(EditorAction::MoveCursorLeft)
                    }
                }
                KeyCode::Right => {
                    if has_shift {
                        Some(EditorAction::SelectRight)
                    } else {
                        Some(EditorAction::MoveCursorRight)
                    }
                }
                KeyCode::Up => {
                    if has_shift {
                        Some(EditorAction::SelectUp)
                    } else {
                        Some(EditorAction::MoveCursorUp)
                    }
                }
                KeyCode::Down => {
                    if has_shift {
                        Some(EditorAction::SelectDown)
                    } else {
                        Some(EditorAction::MoveCursorDown)
                    }
                }
                KeyCode::Home => Some(EditorAction::MoveCursorLineStart),
                KeyCode::End => Some(EditorAction::MoveCursorLineEnd),
                _ => None,
            };

            if let Some(action) = editor_action {
                controller.execute_action(action);
                workbench.is_modified = true;
            }
            sync_state(workbench, controller, path_str);
            false
        }
        CtEvent::Resize(_cols, _rows) => false,
        CtEvent::Mouse(_) | CtEvent::Paste(_) | CtEvent::FocusGained | CtEvent::FocusLost => false,
    }
}

fn sync_state(
    workbench: &mut Workbench,
    controller: &EditorController,
    path_str: &Option<String>,
) {
    workbench.set_editor_content(&controller.model.get_value(), path_str.clone());
    let pos = controller.cursors.get_primary().position();
    workbench.set_cursor_info(pos.line, pos.column);
}
