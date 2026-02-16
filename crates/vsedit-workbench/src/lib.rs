//! Main workbench shell — wires together layout, views, statusbar, editor
//! groups, and drives top-level rendering.

use std::collections::HashMap;

use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use vsedit_commands::{CommandRegistration, CommandRegistry};
use vsedit_contextkey::{ContextKeyValue, IContext};
use vsedit_input::{key_input_to_chord, InputEvent, KeyInput};
use vsedit_keybinding_svc::{
    register_default_keybindings, KeybindingResolver, KeybindingRule, KeybindingWeight,
    ResolveResult,
};
use vsedit_keycodes::{KeyCode, KeyCodeChord};
use vsedit_wb_layout::WorkbenchLayout;
use vsedit_wb_statusbar::{register_default_items, StatusBarService};
use vsedit_wb_views::{register_default_containers, ViewContainerLocation, ViewsRegistry};

// ---------------------------------------------------------------------------
// FocusedPart
// ---------------------------------------------------------------------------

/// Which part of the workbench currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPart {
    Editor,
    Sidebar,
    Panel,
    QuickInput,
    CommandPalette,
}

// ---------------------------------------------------------------------------
// WorkbenchAction
// ---------------------------------------------------------------------------

/// Action returned from input handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkbenchAction {
    None,
    Quit,
    ExecuteCommand(String),
    WaitingForChord,
}

// ---------------------------------------------------------------------------
// WorkbenchContext — simple IContext for keybinding resolution
// ---------------------------------------------------------------------------

struct WorkbenchContext {
    values: HashMap<String, ContextKeyValue>,
}

impl WorkbenchContext {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    fn set(&mut self, key: &str, value: ContextKeyValue) {
        self.values.insert(key.to_string(), value);
    }
}

impl IContext for WorkbenchContext {
    fn get_value(&self, key: &str) -> Option<&ContextKeyValue> {
        self.values.get(key)
    }
}

// ---------------------------------------------------------------------------
// Workbench
// ---------------------------------------------------------------------------

/// The main workbench state, owning all top-level subsystems.
pub struct Workbench {
    started: bool,
    pub layout: WorkbenchLayout,
    pub statusbar: StatusBarService,
    pub commands: CommandRegistry,
    pub keybindings: KeybindingResolver,
    pub views: ViewsRegistry,
    pub focused: FocusedPart,
    context: WorkbenchContext,
    pending_chords: Vec<KeyCodeChord>,
    // Keep registrations alive so commands stay registered.
    _registrations: Vec<CommandRegistration>,
    /// Lines of the currently open file, if any.
    editor_content: Option<Vec<String>>,
    /// Path of the currently open file, if any.
    file_path: Option<String>,
    /// Whether the editor content has been modified since last save.
    pub is_modified: bool,
    /// Current cursor line (1-based).
    cursor_line: u32,
    /// Current cursor column (1-based).
    cursor_col: u32,
}

impl Workbench {
    /// Create a new workbench, initializing all subsystems and registering
    /// default keybindings, commands, and status bar items.
    pub fn new() -> Self {
        let layout = WorkbenchLayout::new();

        let mut statusbar = StatusBarService::new();
        register_default_items(&mut statusbar);

        let commands = CommandRegistry::new();

        let mut keybindings = KeybindingResolver::new();
        register_default_keybindings(&mut keybindings);
        // Workbench-level keybindings
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyB,
            )),
            command: "workbench.action.toggleSidebarVisibility".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyJ,
            )),
            command: "workbench.action.togglePanel".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyQ,
            )),
            command: "workbench.action.quit".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
        });

        let mut views = ViewsRegistry::new();
        register_default_containers(&mut views);
        views.set_active_container(ViewContainerLocation::Sidebar, "workbench.view.explorer");

        // Register built-in commands
        let regs = vec![
            commands.register(
                "workbench.action.toggleSidebarVisibility",
                Box::new(|_| Ok(None)),
            ),
            commands.register("workbench.action.togglePanel", Box::new(|_| Ok(None))),
            commands.register("workbench.action.quit", Box::new(|_| Ok(None))),
        ];

        let mut context = WorkbenchContext::new();
        context.set("editorTextFocus", ContextKeyValue::Bool(false));

        Self {
            started: false,
            layout,
            statusbar,
            commands,
            keybindings,
            views,
            focused: FocusedPart::Editor,
            context,
            pending_chords: Vec::new(),
            _registrations: regs,
            editor_content: None,
            file_path: None,
            is_modified: false,
            cursor_line: 1,
            cursor_col: 1,
        }
    }

    /// Mark the workbench as started.
    pub fn start(&mut self) {
        self.started = true;
    }

    /// Whether the workbench has been started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Set the editor content and file path for rendering.
    pub fn set_editor_content(&mut self, content: &str, path: Option<String>) {
        self.editor_content = Some(content.lines().map(|l| l.to_string()).collect());
        self.file_path = path;
    }

    /// Update cursor position displayed in the status bar.
    pub fn set_cursor_info(&mut self, line: u32, col: u32) {
        self.cursor_line = line;
        self.cursor_col = col;
        self.statusbar
            .update_item("statusbar.lineColumn", &format!("Ln {}, Col {}", line, col));
    }

    /// Render the entire workbench into the given frame.
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let result = self.layout.compute(area);

        // Titlebar / menubar
        if let Some(menubar) = result.menubar {
            let title_text = match &self.file_path {
                Some(path) => {
                    let modified = if self.is_modified { " ●" } else { "" };
                    format!("{}{} — vsedit", path, modified)
                }
                None => "vsedit".to_string(),
            };
            let title = Paragraph::new(Line::from(vec![
                Span::styled(title_text, Style::default().fg(Color::Cyan)),
            ]))
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::DarkGray));
            frame.render_widget(title, menubar);
        }

        // Activity bar
        if let Some(ab) = result.activity_bar {
            let containers = self.views.get_containers(ViewContainerLocation::Sidebar);
            let icons: Vec<Line> = containers
                .iter()
                .map(|c| {
                    let icon_char = match c.id.as_str() {
                        "workbench.view.explorer" => "📁",
                        "workbench.view.search" => "🔍",
                        "workbench.view.scm" => "⎇",
                        "workbench.view.debug" => "▶",
                        "workbench.view.extensions" => "⊞",
                        _ => "·",
                    };
                    Line::from(icon_char)
                })
                .collect();
            let activity = Paragraph::new(icons).style(Style::default().bg(Color::DarkGray));
            frame.render_widget(activity, ab);
        }

        // Sidebar
        if let Some(sb) = result.sidebar {
            let active_title = self
                .views
                .get_active_container(ViewContainerLocation::Sidebar)
                .and_then(|id| {
                    self.views
                        .get_containers(ViewContainerLocation::Sidebar)
                        .into_iter()
                        .find(|c| c.id == id)
                        .map(|c| c.title.to_uppercase())
                })
                .unwrap_or_else(|| "EXPLORER".to_string());
            let sidebar = Paragraph::new(active_title)
                .block(Block::default().borders(Borders::RIGHT))
                .style(Style::default().bg(Color::Black));
            frame.render_widget(sidebar, sb);
        }

        // Editor area
        {
            match &self.editor_content {
                Some(lines) if !lines.is_empty() || self.file_path.is_some() => {
                    let num_width = if lines.is_empty() {
                        1
                    } else {
                        lines.len().to_string().len()
                    };
                    let rendered: Vec<Line> = if lines.is_empty() {
                        vec![Line::from(Span::styled(
                            "  (empty file)",
                            Style::default().fg(Color::DarkGray),
                        ))]
                    } else {
                        lines
                            .iter()
                            .enumerate()
                            .map(|(i, line)| {
                                Line::from(vec![
                                    Span::styled(
                                        format!(" {:>width$} ", i + 1, width = num_width),
                                        Style::default().fg(Color::DarkGray),
                                    ),
                                    Span::raw(line.as_str()),
                                ])
                            })
                            .collect()
                    };
                    let editor = Paragraph::new(rendered);
                    frame.render_widget(editor, result.editor);
                }
                _ => {
                    let editor = Paragraph::new("No editors open")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(editor, result.editor);
                }
            }
        }

        // Panel
        if let Some(panel) = result.panel {
            let panel_widget = Paragraph::new("TERMINAL")
                .block(Block::default().borders(Borders::TOP))
                .style(Style::default().bg(Color::Black));
            frame.render_widget(panel_widget, panel);
        }

        // Statusbar
        {
            let left_items = self.statusbar.get_left_items();
            let right_items = self.statusbar.get_right_items();

            let mut left_spans: Vec<Span> = Vec::new();
            for item in &left_items {
                if !item.text.is_empty() {
                    if !left_spans.is_empty() {
                        left_spans.push(Span::raw(" "));
                    }
                    left_spans.push(Span::raw(item.text.clone()));
                }
            }

            let mut right_spans: Vec<Span> = Vec::new();
            for item in &right_items {
                if !item.text.is_empty() {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::raw(" "));
                    }
                    right_spans.push(Span::raw(item.text.clone()));
                }
            }

            let right_text: String = right_spans.iter().map(|s| s.content.as_ref()).collect();
            let right_width = right_text.len() as u16;
            let bar_width = result.statusbar.width;

            // Render left-aligned items
            let left_para = Paragraph::new(Line::from(left_spans))
                .style(Style::default().bg(Color::Blue).fg(Color::White));
            frame.render_widget(left_para, result.statusbar);

            // Render right-aligned items in the rightmost area
            if right_width > 0 && bar_width > right_width {
                let right_area = ratatui::layout::Rect::new(
                    result.statusbar.x + bar_width - right_width,
                    result.statusbar.y,
                    right_width,
                    result.statusbar.height,
                );
                let right_para = Paragraph::new(Line::from(right_spans))
                    .style(Style::default().bg(Color::Blue).fg(Color::White));
                frame.render_widget(right_para, right_area);
            }
        }
    }

    /// Handle an input event, returning the resulting action.
    pub fn handle_input(&mut self, input: InputEvent) -> WorkbenchAction {
        match input {
            InputEvent::Key(key) => self.handle_key(key),
            _ => WorkbenchAction::None,
        }
    }

    fn handle_key(&mut self, key: KeyInput) -> WorkbenchAction {
        let chord = key_input_to_chord(key);
        self.pending_chords.push(chord);

        let result = self.keybindings.resolve(&self.context, &self.pending_chords);

        match result {
            ResolveResult::CommandMatch { command, .. } => {
                self.pending_chords.clear();
                WorkbenchAction::ExecuteCommand(command)
            }
            ResolveResult::MoreChordsNeeded => WorkbenchAction::WaitingForChord,
            ResolveResult::NoMatch => {
                self.pending_chords.clear();
                WorkbenchAction::None
            }
        }
    }

    /// Execute a known workbench command by ID.
    pub fn execute_command(&mut self, command_id: &str) {
        match command_id {
            "workbench.action.toggleSidebarVisibility" => {
                self.layout.toggle_sidebar();
            }
            "workbench.action.togglePanel" => {
                self.layout.toggle_panel();
            }
            "workbench.action.quit" => {
                // Handled by caller checking WorkbenchAction::Quit
            }
            _ => {
                let _ = self.commands.execute(command_id, vec![]);
            }
        }
    }
}

impl Default for Workbench {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use vsedit_keycodes::KeyCode;
    use vsedit_wb_layout::Part;

    fn make_key(code: KeyCode, ctrl: bool, shift: bool) -> InputEvent {
        InputEvent::Key(KeyInput {
            key_code: code,
            ctrl,
            shift,
            alt: false,
            meta: false,
        })
    }

    #[test]
    fn workbench_lifecycle() {
        let mut wb = Workbench::new();
        assert!(!wb.is_started());
        wb.start();
        assert!(wb.is_started());
    }

    #[test]
    fn default_focus_is_editor() {
        let wb = Workbench::new();
        assert_eq!(wb.focused, FocusedPart::Editor);
    }

    #[test]
    fn default_commands_registered() {
        let wb = Workbench::new();
        assert!(wb.commands.has("workbench.action.toggleSidebarVisibility"));
        assert!(wb.commands.has("workbench.action.togglePanel"));
        assert!(wb.commands.has("workbench.action.quit"));
    }

    #[test]
    fn default_statusbar_items() {
        let wb = Workbench::new();
        let left = wb.statusbar.get_left_items();
        assert!(!left.is_empty());
        let right = wb.statusbar.get_right_items();
        assert!(!right.is_empty());
    }

    #[test]
    fn default_views_registered() {
        let wb = Workbench::new();
        let sidebar = wb.views.get_containers(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar.len(), 5);
        assert_eq!(
            wb.views
                .get_active_container(ViewContainerLocation::Sidebar),
            Some("workbench.view.explorer")
        );
    }

    #[test]
    fn toggle_sidebar_via_command() {
        let mut wb = Workbench::new();
        assert!(wb.layout.is_part_visible(Part::Sidebar));
        wb.execute_command("workbench.action.toggleSidebarVisibility");
        assert!(!wb.layout.is_part_visible(Part::Sidebar));
        wb.execute_command("workbench.action.toggleSidebarVisibility");
        assert!(wb.layout.is_part_visible(Part::Sidebar));
    }

    #[test]
    fn toggle_panel_via_command() {
        let mut wb = Workbench::new();
        assert!(wb.layout.is_part_visible(Part::Panel));
        wb.execute_command("workbench.action.togglePanel");
        assert!(!wb.layout.is_part_visible(Part::Panel));
    }

    #[test]
    fn handle_keybinding_toggle_sidebar() {
        let mut wb = Workbench::new();
        // Ctrl+B should toggle sidebar
        let action = wb.handle_input(make_key(KeyCode::KeyB, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand(
                "workbench.action.toggleSidebarVisibility".to_string()
            )
        );
    }

    #[test]
    fn handle_keybinding_quit() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyQ, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.quit".to_string())
        );
    }

    #[test]
    fn handle_unbound_key_returns_none() {
        let mut wb = Workbench::new();
        // F24 with no modifiers is unlikely to be bound
        let action = wb.handle_input(make_key(KeyCode::F24, false, false));
        assert_eq!(action, WorkbenchAction::None);
    }

    #[test]
    fn handle_mouse_event_returns_none() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(InputEvent::Mouse(vsedit_input::MouseInput {
            action: vsedit_input::MouseAction::Down,
            button: vsedit_input::MouseButton::Left,
            column: 0,
            row: 0,
            ctrl: false,
            shift: false,
            alt: false,
        }));
        assert_eq!(action, WorkbenchAction::None);
    }

    #[test]
    fn render_does_not_panic() {
        let wb = Workbench::new();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    #[test]
    fn render_with_sidebar_hidden() {
        let mut wb = Workbench::new();
        wb.layout.toggle_sidebar();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    #[test]
    fn render_with_panel_hidden() {
        let mut wb = Workbench::new();
        wb.layout.toggle_panel();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    #[test]
    fn render_small_terminal() {
        let wb = Workbench::new();
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    #[test]
    fn workbench_action_variants() {
        let none = WorkbenchAction::None;
        let quit = WorkbenchAction::Quit;
        let exec = WorkbenchAction::ExecuteCommand("test".to_string());
        let waiting = WorkbenchAction::WaitingForChord;

        assert_ne!(none, quit);
        assert_ne!(exec, waiting);
        assert_eq!(none.clone(), WorkbenchAction::None);
    }

    #[test]
    fn focused_part_variants() {
        assert_ne!(FocusedPart::Editor, FocusedPart::Sidebar);
        assert_ne!(FocusedPart::Panel, FocusedPart::QuickInput);
        assert_ne!(FocusedPart::CommandPalette, FocusedPart::Editor);
    }
}
