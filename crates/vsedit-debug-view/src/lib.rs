//! Debug view and features.
//!
//! Provides a debug sidebar with variables, call stack, breakpoints,
//! and watch expressions — rendered via ratatui.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

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
}

impl DebugVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>, var_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            var_type: var_type.into(),
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<DebugVariable>) -> Self {
        self.children = children;
        self
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
        self.render_sections(content_area, buf);
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
        let line = Line::from(vec![Span::styled(
            state_label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]);
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
                .map(|v| format!("{}: {} = {}", v.var_type, v.name, v.value))
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
                    format!("{} {}:{}", icon, bp.file_path, bp.line)
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
}
