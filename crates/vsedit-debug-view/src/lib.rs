//! Debug view and features.
//!
//! Provides a debug sidebar with variables, call stack, breakpoints,
//! and watch expressions — rendered via ratatui. Integrates with the
//! `vsedit-debug` DAP client for real debugging data.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

pub use vsedit_debug::console::{DebugConsole, DebugConsoleEntry, OutputCategory};
pub use vsedit_debug::types::{Scope, Variable as DapVariable};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Current debug session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    Inactive,
    Running,
    Paused,
    Stopped,
}

/// A section in the debug sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugSection {
    Variables,
    CallStack,
    Breakpoints,
    Watch,
}

/// A debug variable displayed in the variables tree.
#[derive(Debug, Clone)]
pub struct DebugVariable {
    pub name: String,
    pub value: String,
    pub var_type: String,
    pub children: Vec<DebugVariable>,
    /// DAP variables_reference for lazy-loading children.
    pub variables_reference: u64,
    pub expanded: bool,
}

impl DebugVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>, var_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            var_type: var_type.into(),
            children: Vec::new(),
            variables_reference: 0,
            expanded: false,
        }
    }

    pub fn with_children(mut self, children: Vec<DebugVariable>) -> Self {
        self.children = children;
        self
    }

    /// Create from a DAP variable.
    pub fn from_dap(var: &DapVariable) -> Self {
        Self {
            name: var.name.clone(),
            value: var.value.clone(),
            var_type: var.type_name.clone().unwrap_or_default(),
            children: Vec::new(),
            variables_reference: var.variables_reference,
            expanded: false,
        }
    }

    /// Returns true if this variable can be expanded.
    pub fn has_children(&self) -> bool {
        self.variables_reference > 0 || !self.children.is_empty()
    }
}

/// A stack frame in the call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub source_path: String,
    pub line: u32,
    pub column: u32,
}

impl StackFrame {
    pub fn new(id: u64, name: impl Into<String>, source_path: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            id,
            name: name.into(),
            source_path: source_path.into(),
            line,
            column,
        }
    }

    /// Create from a DAP stack frame.
    pub fn from_dap(frame: &vsedit_debug::types::StackFrame) -> Self {
        Self {
            id: frame.id,
            name: frame.name.clone(),
            source_path: frame.source_path.clone().unwrap_or_default(),
            line: frame.line,
            column: frame.column,
        }
    }
}

/// A breakpoint set in source code.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u64,
    pub file_path: String,
    pub line: u32,
    pub enabled: bool,
    pub condition: Option<String>,
    pub hit_count: u32,
}

impl Breakpoint {
    pub fn new(id: u64, file_path: impl Into<String>, line: u32) -> Self {
        Self {
            id,
            file_path: file_path.into(),
            line,
            enabled: true,
            condition: None,
            hit_count: 0,
        }
    }
}

/// Debug toolbar action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Restart,
    Stop,
}

impl DebugAction {
    pub fn label(self) -> &'static str {
        match self {
            DebugAction::Continue => "▶ Continue",
            DebugAction::StepOver => "⤵ Step Over",
            DebugAction::StepIn => "↓ Step In",
            DebugAction::StepOut => "↑ Step Out",
            DebugAction::Restart => "⟳ Restart",
            DebugAction::Stop => "⏹ Stop",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            DebugAction::Continue => "F5",
            DebugAction::StepOver => "F10",
            DebugAction::StepIn => "F11",
            DebugAction::StepOut => "⇧F11",
            DebugAction::Restart => "⇧⌘F5",
            DebugAction::Stop => "⇧F5",
        }
    }
}

// ---------------------------------------------------------------------------
// DebugView
// ---------------------------------------------------------------------------

/// Debug sidebar view with variables, call stack, breakpoints, and watch.
#[derive(Debug, Clone)]
pub struct DebugView {
    pub state: DebugState,
    pub variables: Vec<DebugVariable>,
    pub call_stack: Vec<StackFrame>,
    pub breakpoints: Vec<Breakpoint>,
    pub watches: Vec<String>,
    pub selected_section: DebugSection,
    pub selected_index: usize,
    pub console: DebugConsole,
    pub show_console: bool,
    /// Scopes for the selected stack frame.
    pub scopes: Vec<Scope>,
}

impl DebugView {
    pub fn new() -> Self {
        Self {
            state: DebugState::Inactive,
            variables: Vec::new(),
            call_stack: Vec::new(),
            breakpoints: Vec::new(),
            watches: Vec::new(),
            selected_section: DebugSection::Variables,
            selected_index: 0,
            console: DebugConsole::new(),
            show_console: false,
            scopes: Vec::new(),
        }
    }

    /// Select the next section.
    pub fn next_section(&mut self) {
        self.selected_section = match self.selected_section {
            DebugSection::Variables => DebugSection::CallStack,
            DebugSection::CallStack => DebugSection::Breakpoints,
            DebugSection::Breakpoints => DebugSection::Watch,
            DebugSection::Watch => DebugSection::Variables,
        };
        self.selected_index = 0;
    }

    /// Count items in the currently selected section.
    pub fn current_section_len(&self) -> usize {
        match self.selected_section {
            DebugSection::Variables => self.variables.len(),
            DebugSection::CallStack => self.call_stack.len(),
            DebugSection::Breakpoints => self.breakpoints.len(),
            DebugSection::Watch => self.watches.len(),
        }
    }

    /// Move selection down within the current section.
    pub fn select_next(&mut self) {
        let len = self.current_section_len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1).min(len - 1);
        }
    }

    /// Move selection up within the current section.
    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Toggle a breakpoint's enabled state by index.
    pub fn toggle_breakpoint(&mut self, index: usize) -> bool {
        if let Some(bp) = self.breakpoints.get_mut(index) {
            bp.enabled = !bp.enabled;
            true
        } else {
            false
        }
    }

    /// Toggle expand/collapse of a variable at the given index.
    pub fn toggle_variable_expand(&mut self, index: usize) {
        if let Some(var) = self.variables.get_mut(index) {
            var.expanded = !var.expanded;
        }
    }

    /// Update variables from DAP variables.
    pub fn set_variables_from_dap(&mut self, vars: &[DapVariable]) {
        self.variables = vars.iter().map(|v| DebugVariable::from_dap(v)).collect();
    }

    /// Update call stack from DAP stack frames.
    pub fn set_call_stack_from_dap(&mut self, frames: &[vsedit_debug::types::StackFrame]) {
        self.call_stack = frames.iter().map(|f| StackFrame::from_dap(f)).collect();
    }

    /// Render a status bar segment for debug state.
    pub fn status_bar_text(&self) -> &'static str {
        match self.state {
            DebugState::Inactive => "",
            DebugState::Running => "🐛 Debugging",
            DebugState::Paused => "🐛 Paused",
            DebugState::Stopped => "🐛 Stopped",
        }
    }

    /// Render the debug sidebar.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        // Toolbar (row 0)
        let toolbar_area = Rect { height: 1, ..area };
        self.render_toolbar(toolbar_area, buf);

        // Content sections (remaining rows)
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };

        if self.show_console {
            // Split: top half sections, bottom half console
            let half = content_area.height / 2;
            let sections_area = Rect {
                height: half,
                ..content_area
            };
            self.render_sections(sections_area, buf);

            let console_area = Rect {
                y: content_area.y + half,
                height: content_area.height.saturating_sub(half),
                ..content_area
            };
            self.render_console(console_area, buf);
        } else {
            self.render_sections(content_area, buf);
        }
    }

    fn render_toolbar(&self, area: Rect, buf: &mut Buffer) {
        let state_label = match self.state {
            DebugState::Inactive => "⏹ Inactive",
            DebugState::Running => "▶ Running",
            DebugState::Paused => "⏸ Paused",
            DebugState::Stopped => "⏹ Stopped",
        };
        let color = match self.state {
            DebugState::Running => Color::Green,
            DebugState::Paused => Color::Yellow,
            _ => Color::Gray,
        };

        let mut spans = vec![Span::styled(
            state_label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )];

        // Show toolbar actions when paused
        if self.state == DebugState::Paused {
            let actions = [
                DebugAction::Continue,
                DebugAction::StepOver,
                DebugAction::StepIn,
                DebugAction::StepOut,
                DebugAction::Stop,
            ];
            for action in &actions {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    action.short_label(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        let line = Line::from(spans);
        line.render(area, buf);
    }

    fn render_sections(&self, area: Rect, buf: &mut Buffer) {
        let sections = [
            (DebugSection::Variables, "VARIABLES"),
            (DebugSection::CallStack, "CALL STACK"),
            (DebugSection::Breakpoints, "BREAKPOINTS"),
            (DebugSection::Watch, "WATCH"),
        ];

        let section_height = area.height / 4;
        if section_height == 0 {
            return;
        }

        for (i, (section, title)) in sections.iter().enumerate() {
            let y = area.y + (i as u16) * section_height;
            let h = if i == 3 {
                area.height - 3 * section_height
            } else {
                section_height
            };
            let sect_area = Rect {
                y,
                height: h,
                ..area
            };

            let is_selected = *section == self.selected_section;
            let header_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let header = Line::from(vec![Span::styled(*title, header_style)]);
            let header_area = Rect {
                height: 1,
                ..sect_area
            };
            header.render(header_area, buf);

            let items_area = Rect {
                y: sect_area.y + 1,
                height: sect_area.height.saturating_sub(1),
                ..sect_area
            };
            self.render_section_items(*section, items_area, buf);
        }
    }

    fn render_section_items(&self, section: DebugSection, area: Rect, buf: &mut Buffer) {
        let items: Vec<String> = match section {
            DebugSection::Variables => self
                .variables
                .iter()
                .map(|v| {
                    let expand_icon = if v.has_children() {
                        if v.expanded { "▼ " } else { "▶ " }
                    } else {
                        "  "
                    };
                    format!("{}{}: {} = {}", expand_icon, v.var_type, v.name, v.value)
                })
                .collect(),
            DebugSection::CallStack => self
                .call_stack
                .iter()
                .map(|f| format!("{} ({}:{})", f.name, f.source_path, f.line))
                .collect(),
            DebugSection::Breakpoints => self
                .breakpoints
                .iter()
                .map(|bp| {
                    let icon = if bp.enabled { "●" } else { "○" };
                    match &bp.condition {
                        Some(cond) => format!("{} {}:{} when {}", icon, bp.file_path, bp.line, cond),
                        None => format!("{} {}:{}", icon, bp.file_path, bp.line),
                    }
                })
                .collect(),
            DebugSection::Watch => self.watches.clone(),
        };

        for (i, item) in items.iter().enumerate() {
            if i as u16 >= area.height {
                break;
            }
            let is_selected = section == self.selected_section && i == self.selected_index;
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            let line = Line::from(vec![Span::styled(
                format!("  {}", item),
                style,
            )]);
            let row = Rect {
                y: area.y + i as u16,
                height: 1,
                ..area
            };
            line.render(row, buf);
        }
    }

    fn render_console(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Header
        let header = Line::from(vec![Span::styled(
            "DEBUG CONSOLE",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let header_area = Rect {
            height: 1,
            ..area
        };
        header.render(header_area, buf);

        // Entries
        let entries_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        let entries = self.console.entries();
        let visible_count = entries_area.height as usize;
        let start = entries.len().saturating_sub(visible_count);

        for (i, entry) in entries[start..].iter().enumerate() {
            if i as u16 >= entries_area.height {
                break;
            }
            let (prefix, color) = match entry {
                DebugConsoleEntry::Input(_) => ("> ", Color::Cyan),
                DebugConsoleEntry::Output(_, OutputCategory::Stderr) => ("", Color::Red),
                DebugConsoleEntry::Output(_, OutputCategory::Stdout) => ("", Color::White),
                DebugConsoleEntry::Output(_, _) => ("", Color::Gray),
            };
            let text = format!("{}{}", prefix, entry.text().trim_end());
            let line = Line::from(vec![Span::styled(text, Style::default().fg(color))]);
            let row = Rect {
                y: entries_area.y + i as u16,
                height: 1,
                ..entries_area
            };
            line.render(row, buf);
        }
    }

    /// Returns true if variables is empty.
    pub fn is_variables_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Get the first variable, if any.
    pub fn first_variable(&self) -> Option<&DebugVariable> {
        self.variables.first()
    }

    /// Get the last variable, if any.
    pub fn last_variable(&self) -> Option<&DebugVariable> {
        self.variables.last()
    }

    /// Retain only variables matching the predicate.
    pub fn retain_variables(&mut self, f: impl Fn(&DebugVariable) -> bool) {
        self.variables.retain(|item| f(item));
    }

    /// Returns true if call_stack is empty.
    pub fn is_call_stack_empty(&self) -> bool {
        self.call_stack.is_empty()
    }

    /// Get the first call_stack, if any.
    pub fn first_call_stack(&self) -> Option<&StackFrame> {
        self.call_stack.first()
    }

    /// Get the last call_stack, if any.
    pub fn last_call_stack(&self) -> Option<&StackFrame> {
        self.call_stack.last()
    }

    /// Retain only call_stack matching the predicate.
    pub fn retain_call_stack(&mut self, f: impl Fn(&StackFrame) -> bool) {
        self.call_stack.retain(|item| f(item));
    }

    /// Returns true if breakpoints is empty.
    pub fn is_breakpoints_empty(&self) -> bool {
        self.breakpoints.is_empty()
    }

    /// Get the first breakpoint, if any.
    pub fn first_breakpoint(&self) -> Option<&Breakpoint> {
        self.breakpoints.first()
    }

    /// Get the last breakpoint, if any.
    pub fn last_breakpoint(&self) -> Option<&Breakpoint> {
        self.breakpoints.last()
    }

    /// Retain only breakpoints matching the predicate.
    pub fn retain_breakpoints(&mut self, f: impl Fn(&Breakpoint) -> bool) {
        self.breakpoints.retain(|item| f(item));
    }

    /// Returns true if watches is empty.
    pub fn is_watches_empty(&self) -> bool {
        self.watches.is_empty()
    }

    /// Get the first watche, if any.
    pub fn first_watche(&self) -> Option<&String> {
        self.watches.first()
    }

    /// Get the last watche, if any.
    pub fn last_watche(&self) -> Option<&String> {
        self.watches.last()
    }

    /// Retain only watches matching the predicate.
    pub fn retain_watches(&mut self, f: impl Fn(&String) -> bool) {
        self.watches.retain(|item| f(item));
    }
}

impl Default for DebugView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = DebugView::new();
        assert_eq!(v.state, DebugState::Inactive);
        assert!(v.variables.is_empty());
    }

    #[test]
    fn debug_variable_with_children() {
        let child = DebugVariable::new("x", "42", "i32");
        let parent = DebugVariable::new("obj", "{...}", "MyStruct")
            .with_children(vec![child]);
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].name, "x");
    }

    #[test]
    fn stack_frame_creation() {
        let frame = StackFrame::new(1, "main", "src/main.rs", 10, 5);
        assert_eq!(frame.id, 1);
        assert_eq!(frame.line, 10);
    }

    #[test]
    fn breakpoint_creation() {
        let bp = Breakpoint::new(1, "src/lib.rs", 42);
        assert!(bp.enabled);
        assert_eq!(bp.hit_count, 0);
    }

    #[test]
    fn next_section_cycles() {
        let mut v = DebugView::new();
        assert_eq!(v.selected_section, DebugSection::Variables);
        v.next_section();
        assert_eq!(v.selected_section, DebugSection::CallStack);
        v.next_section();
        assert_eq!(v.selected_section, DebugSection::Breakpoints);
        v.next_section();
        assert_eq!(v.selected_section, DebugSection::Watch);
        v.next_section();
        assert_eq!(v.selected_section, DebugSection::Variables);
    }

    #[test]
    fn select_next_clamps() {
        let mut v = DebugView::new();
        v.variables.push(DebugVariable::new("x", "1", "i32"));
        v.variables.push(DebugVariable::new("y", "2", "i32"));
        v.select_next();
        assert_eq!(v.selected_index, 1);
        v.select_next();
        assert_eq!(v.selected_index, 1);
    }

    #[test]
    fn select_previous_clamps() {
        let mut v = DebugView::new();
        v.select_previous();
        assert_eq!(v.selected_index, 0);
    }

    #[test]
    fn toggle_breakpoint() {
        let mut v = DebugView::new();
        v.breakpoints.push(Breakpoint::new(1, "test.rs", 10));
        assert!(v.breakpoints[0].enabled);
        v.toggle_breakpoint(0);
        assert!(!v.breakpoints[0].enabled);
        assert!(!v.toggle_breakpoint(5));
    }

    #[test]
    fn current_section_len() {
        let mut v = DebugView::new();
        v.variables.push(DebugVariable::new("x", "1", "i32"));
        assert_eq!(v.current_section_len(), 1);
        v.selected_section = DebugSection::CallStack;
        assert_eq!(v.current_section_len(), 0);
    }

    #[test]
    fn render_does_not_panic() {
        let mut v = DebugView::new();
        v.state = DebugState::Paused;
        v.variables.push(DebugVariable::new("x", "42", "i32"));
        v.call_stack.push(StackFrame::new(1, "main", "main.rs", 1, 1));
        v.breakpoints.push(Breakpoint::new(1, "lib.rs", 10));
        v.watches.push("expr".to_string());
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_small_area_no_panic() {
        let v = DebugView::new();
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let v = DebugView::default();
        assert_eq!(v.state, DebugState::Inactive);
    }

    #[test]
    fn eq_debugstate_same() {
        assert_eq!(DebugState::Inactive, DebugState::Inactive);
    }

    #[test]
    fn ne_debugstate_diff() {
        assert_ne!(DebugState::Inactive, DebugState::Running);
    }

    #[test]
    fn eq_debugsection_same() {
        assert_eq!(DebugSection::Variables, DebugSection::Variables);
    }

    #[test]
    fn ne_debugsection_diff() {
        assert_ne!(DebugSection::Variables, DebugSection::CallStack);
    }

    #[test]
    fn debug_action_labels() {
        assert_eq!(DebugAction::Continue.label(), "▶ Continue");
        assert_eq!(DebugAction::StepOver.short_label(), "F10");
        assert_eq!(DebugAction::Stop.short_label(), "⇧F5");
    }

    #[test]
    fn status_bar_text() {
        let mut v = DebugView::new();
        assert_eq!(v.status_bar_text(), "");
        v.state = DebugState::Running;
        assert_eq!(v.status_bar_text(), "🐛 Debugging");
        v.state = DebugState::Paused;
        assert_eq!(v.status_bar_text(), "🐛 Paused");
    }

    #[test]
    fn variable_from_dap() {
        let dap_var = DapVariable::new("x", "42").with_type("i32");
        let view_var = DebugVariable::from_dap(&dap_var);
        assert_eq!(view_var.name, "x");
        assert_eq!(view_var.value, "42");
        assert_eq!(view_var.var_type, "i32");
    }

    #[test]
    fn variable_expand_toggle() {
        let mut v = DebugView::new();
        let mut var = DebugVariable::new("obj", "{...}", "Object");
        var.variables_reference = 10;
        v.variables.push(var);
        assert!(!v.variables[0].expanded);
        v.toggle_variable_expand(0);
        assert!(v.variables[0].expanded);
    }

    #[test]
    fn set_variables_from_dap() {
        let mut v = DebugView::new();
        let dap_vars = vec![
            DapVariable::new("x", "1"),
            DapVariable::new("y", "2"),
        ];
        v.set_variables_from_dap(&dap_vars);
        assert_eq!(v.variables.len(), 2);
    }

    #[test]
    fn set_call_stack_from_dap() {
        let mut v = DebugView::new();
        let frames = vec![
            vsedit_debug::types::StackFrame::new(1, "main", 10, 1)
                .with_source("/app/main.rs"),
        ];
        v.set_call_stack_from_dap(&frames);
        assert_eq!(v.call_stack.len(), 1);
        assert_eq!(v.call_stack[0].name, "main");
    }

    #[test]
    fn render_with_console() {
        let mut v = DebugView::new();
        v.show_console = true;
        v.console.add_input("x + 1");
        v.console.add_output("42", OutputCategory::Console);
        v.console.add_output("error msg", OutputCategory::Stderr);
        let area = Rect::new(0, 0, 60, 30);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_conditional_breakpoint_display() {
        let mut v = DebugView::new();
        let mut bp = Breakpoint::new(1, "main.rs", 10);
        bp.condition = Some("x > 5".into());
        v.breakpoints.push(bp);
        v.selected_section = DebugSection::Breakpoints;
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_paused_with_toolbar_actions() {
        let mut v = DebugView::new();
        v.state = DebugState::Paused;
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn variable_has_children_check() {
        let simple = DebugVariable::new("x", "42", "i32");
        assert!(!simple.has_children());

        let mut with_ref = DebugVariable::new("obj", "{...}", "Struct");
        with_ref.variables_reference = 10;
        assert!(with_ref.has_children());

        let with_kids = DebugVariable::new("arr", "[...]", "Vec")
            .with_children(vec![DebugVariable::new("0", "1", "i32")]);
        assert!(with_kids.has_children());
    }

    #[test]
    fn stack_frame_from_dap_conversion() {
        let dap_frame = vsedit_debug::types::StackFrame::new(5, "foo", 100, 3)
            .with_source("/src/foo.rs");
        let view_frame = StackFrame::from_dap(&dap_frame);
        assert_eq!(view_frame.id, 5);
        assert_eq!(view_frame.name, "foo");
        assert_eq!(view_frame.source_path, "/src/foo.rs");
        assert_eq!(view_frame.line, 100);
    }
}
