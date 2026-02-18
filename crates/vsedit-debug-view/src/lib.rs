//! Debug view and features.
//!
//! Provides a debug sidebar with variables, call stack, breakpoints,
//! and watch expressions — rendered via ratatui. Integrates with the
//! `vsedit-debug` DAP client for real debugging data.

use std::collections::HashMap;
use std::fmt;

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

/// Manages a collection of breakpoints with add, remove, toggle, and
/// query operations.
#[derive(Debug, Clone, Default)]
pub struct BreakpointManager {
    breakpoints: Vec<Breakpoint>,
    next_id: u64,
}

impl BreakpointManager {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a breakpoint and return its assigned id.
    pub fn add(&mut self, file_path: impl Into<String>, line: u32) -> u64 {
        let id = self.next_id;
        self.breakpoints.push(Breakpoint::new(id, file_path, line));
        self.next_id += 1;
        id
    }

    /// Remove a breakpoint by id. Returns `true` if it existed.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.breakpoints.len();
        self.breakpoints.retain(|bp| bp.id != id);
        self.breakpoints.len() < before
    }

    /// Toggle the enabled state of a breakpoint by id.
    pub fn toggle_enabled(&mut self, id: u64) -> bool {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = !bp.enabled;
            true
        } else {
            false
        }
    }

    /// Find all breakpoints in a given file.
    pub fn find_by_file(&self, file_path: &str) -> Vec<&Breakpoint> {
        self.breakpoints
            .iter()
            .filter(|bp| bp.file_path == file_path)
            .collect()
    }

    /// Find a breakpoint at a specific file and line.
    pub fn find_by_file_line(&self, file_path: &str, line: u32) -> Option<&Breakpoint> {
        self.breakpoints
            .iter()
            .find(|bp| bp.file_path == file_path && bp.line == line)
    }

    /// Count of enabled breakpoints.
    pub fn count_enabled(&self) -> usize {
        self.breakpoints.iter().filter(|bp| bp.enabled).count()
    }

    /// Total breakpoint count.
    pub fn count(&self) -> usize {
        self.breakpoints.len()
    }
}

// ---------------------------------------------------------------------------
// CallStackFrameInfo
// ---------------------------------------------------------------------------

/// Extended call stack frame with source location and module metadata.
#[derive(Debug, Clone)]
pub struct CallStackFrameInfo {
    pub frame_id: u64,
    pub function_name: String,
    pub source_file: Option<String>,
    pub line: u32,
    pub column: u32,
    pub module_name: Option<String>,
    pub is_external: bool,
}

impl CallStackFrameInfo {
    pub fn new(
        frame_id: u64,
        function_name: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        Self {
            frame_id,
            function_name: function_name.into(),
            source_file: None,
            line,
            column,
            module_name: None,
            is_external: false,
        }
    }

    /// Display the source location, e.g. `"main.rs:42:1"`.
    /// Returns `"<unknown>"` when no source file is set.
    pub fn display_location(&self) -> String {
        match &self.source_file {
            Some(path) => format!("{}:{}:{}", path, self.line, self.column),
            None => "<unknown>".to_string(),
        }
    }

    /// Return the short function name (last segment after `::` if any).
    pub fn short_name(&self) -> &str {
        self.function_name
            .rsplit("::")
            .next()
            .unwrap_or(&self.function_name)
    }

    /// Returns `true` when this frame represents user (non-external) code.
    pub fn is_user_code(&self) -> bool {
        !self.is_external
    }
}

// ---------------------------------------------------------------------------
// VariableInspector
// ---------------------------------------------------------------------------

/// Tracks expand/collapse state for a tree of debug variables and provides
/// a flattened view suitable for list rendering.
#[derive(Debug, Clone, Default)]
pub struct VariableInspector {
    variables: Vec<DebugVariable>,
    expanded_paths: Vec<String>,
}

impl VariableInspector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the variable tree.
    pub fn set_variables(&mut self, variables: Vec<DebugVariable>) {
        self.variables = variables;
    }

    /// Toggle a variable path between expanded and collapsed.
    pub fn toggle_path(&mut self, path: &str) {
        if let Some(idx) = self.expanded_paths.iter().position(|p| p == path) {
            self.expanded_paths.remove(idx);
        } else {
            self.expanded_paths.push(path.to_string());
        }
    }

    /// Returns `true` if the given path is currently expanded.
    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded_paths.iter().any(|p| p == path)
    }

    /// Total number of top-level variables.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Produce a flat list of `(depth, &DebugVariable)` pairs by walking the
    /// tree and expanding nodes whose path is in `expanded_paths`.
    pub fn flatten(&self) -> Vec<(usize, &DebugVariable)> {
        let mut out = Vec::new();
        for var in &self.variables {
            self.flatten_rec(var, &var.name, 0, &mut out);
        }
        out
    }

    fn flatten_rec<'a>(
        &'a self,
        var: &'a DebugVariable,
        path: &str,
        depth: usize,
        out: &mut Vec<(usize, &'a DebugVariable)>,
    ) {
        out.push((depth, var));
        if self.is_expanded(path) {
            for child in &var.children {
                let child_path = format!("{}.{}", path, child.name);
                self.flatten_rec(child, &child_path, depth + 1, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BreakpointList
// ---------------------------------------------------------------------------

/// A list of breakpoints with selection and bulk-toggle support.
#[derive(Debug, Clone, Default)]
pub struct BreakpointList {
    breakpoints: Vec<Breakpoint>,
    selected: Option<usize>,
}

impl BreakpointList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a breakpoint to the list.
    pub fn add(&mut self, bp: Breakpoint) {
        self.breakpoints.push(bp);
        if self.selected.is_none() {
            self.selected = Some(0);
        }
    }

    /// Remove a breakpoint by id. Returns `true` if it existed.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.breakpoints.len();
        self.breakpoints.retain(|bp| bp.id != id);
        let removed = self.breakpoints.len() < before;
        if removed {
            // Fix up selected index
            if self.breakpoints.is_empty() {
                self.selected = None;
            } else if let Some(sel) = self.selected {
                if sel >= self.breakpoints.len() {
                    self.selected = Some(self.breakpoints.len() - 1);
                }
            }
        }
        removed
    }

    /// Toggle the enabled state of a breakpoint by id.
    pub fn toggle(&mut self, id: u64) -> bool {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = !bp.enabled;
            true
        } else {
            false
        }
    }

    /// Enable all breakpoints.
    pub fn enable_all(&mut self) {
        for bp in &mut self.breakpoints {
            bp.enabled = true;
        }
    }

    /// Disable all breakpoints.
    pub fn disable_all(&mut self) {
        for bp in &mut self.breakpoints {
            bp.enabled = false;
        }
    }

    /// Number of enabled breakpoints.
    pub fn enabled_count(&self) -> usize {
        self.breakpoints.iter().filter(|bp| bp.enabled).count()
    }

    /// Move the selection to the next breakpoint.
    pub fn select_next(&mut self) {
        if self.breakpoints.is_empty() {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) if i + 1 < self.breakpoints.len() => i + 1,
            Some(i) => i,
            None => 0,
        });
    }

    /// Move the selection to the previous breakpoint.
    pub fn select_previous(&mut self) {
        if self.breakpoints.is_empty() {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) => i.saturating_sub(1),
            None => 0,
        });
    }

    /// Return a reference to the currently selected breakpoint.
    pub fn selected_breakpoint(&self) -> Option<&Breakpoint> {
        self.selected.and_then(|i| self.breakpoints.get(i))
    }

    /// Return all breakpoints whose file path matches `path`.
    pub fn breakpoints_for_file(&self, path: &str) -> Vec<&Breakpoint> {
        self.breakpoints
            .iter()
            .filter(|bp| bp.file_path == path)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// WatchExpression — watch expression evaluation
// ---------------------------------------------------------------------------

/// Evaluation result of a watch expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchResult {
    Value(String),
    Error(String),
    Pending,
}

/// A watch expression tracked in the debug view.
#[derive(Debug, Clone)]
pub struct WatchExpression {
    pub expression: String,
    pub result: WatchResult,
    pub eval_count: u32,
}

impl WatchExpression {
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            result: WatchResult::Pending,
            eval_count: 0,
        }
    }

    /// Record an evaluation result.
    pub fn set_result(&mut self, value: impl Into<String>) {
        self.result = WatchResult::Value(value.into());
        self.eval_count += 1;
    }

    /// Record an evaluation error.
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.result = WatchResult::Error(error.into());
        self.eval_count += 1;
    }

    pub fn is_pending(&self) -> bool {
        self.result == WatchResult::Pending
    }

    pub fn is_error(&self) -> bool {
        matches!(self.result, WatchResult::Error(_))
    }

    /// Display text for rendering.
    pub fn display_text(&self) -> String {
        match &self.result {
            WatchResult::Value(v) => format!("{} = {}", self.expression, v),
            WatchResult::Error(e) => format!("{} ⚠ {}", self.expression, e),
            WatchResult::Pending => format!("{} …", self.expression),
        }
    }
}

/// Manages a list of watch expressions.
#[derive(Debug, Clone, Default)]
pub struct WatchExpressionList {
    expressions: Vec<WatchExpression>,
    selected: Option<usize>,
}

impl WatchExpressionList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, expr: impl Into<String>) {
        self.expressions.push(WatchExpression::new(expr));
        if self.selected.is_none() {
            self.selected = Some(0);
        }
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.expressions.len() {
            self.expressions.remove(index);
            if self.expressions.is_empty() {
                self.selected = None;
            } else if let Some(sel) = self.selected {
                if sel >= self.expressions.len() {
                    self.selected = Some(self.expressions.len() - 1);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn get(&self, index: usize) -> Option<&WatchExpression> {
        self.expressions.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut WatchExpression> {
        self.expressions.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.expressions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.expressions.is_empty()
    }

    /// Reset all expressions to pending.
    pub fn reset_all(&mut self) {
        for expr in &mut self.expressions {
            expr.result = WatchResult::Pending;
        }
    }

    /// Count of expressions with errors.
    pub fn error_count(&self) -> usize {
        self.expressions.iter().filter(|e| e.is_error()).count()
    }
}

// ---------------------------------------------------------------------------
// DebugConsoleHistory — command history for the debug console
// ---------------------------------------------------------------------------

/// Tracks debug console command history with navigation.
#[derive(Debug, Clone, Default)]
pub struct DebugConsoleHistory {
    entries: Vec<String>,
    cursor: Option<usize>,
    max_entries: usize,
}

impl DebugConsoleHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: None,
            max_entries,
        }
    }

    /// Push a command into history, resetting the cursor.
    pub fn push(&mut self, command: impl Into<String>) {
        let cmd = command.into();
        if cmd.is_empty() {
            return;
        }
        // Deduplicate consecutive
        if self.entries.last().map(|s| s.as_str()) == Some(&cmd) {
            self.cursor = None;
            return;
        }
        self.entries.push(cmd);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    /// Navigate up (older). Returns the command at the cursor.
    pub fn up(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            Some(i) => i.saturating_sub(1),
            None => self.entries.len() - 1,
        };
        self.cursor = Some(idx);
        self.entries.get(idx).map(|s| s.as_str())
    }

    /// Navigate down (newer). Returns the command at the cursor, or None if at end.
    pub fn down(&mut self) -> Option<&str> {
        match self.cursor {
            Some(i) if i + 1 < self.entries.len() => {
                self.cursor = Some(i + 1);
                self.entries.get(i + 1).map(|s| s.as_str())
            }
            _ => {
                self.cursor = None;
                None
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = None;
    }

    /// Search history for entries containing the query.
    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.contains(query))
            .map(|e| e.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// BreakpointCondition — conditional breakpoints
// ---------------------------------------------------------------------------

/// Type of breakpoint condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionKind {
    /// Break when expression evaluates to true.
    Expression(String),
    /// Break when hit count reaches value.
    HitCount(u32),
    /// Break when log message pattern matches.
    LogMessage(String),
}

/// A condition attached to a breakpoint.
#[derive(Debug, Clone)]
pub struct BreakpointCondition {
    pub kind: ConditionKind,
    pub enabled: bool,
    current_hits: u32,
}

impl BreakpointCondition {
    pub fn expression(expr: impl Into<String>) -> Self {
        Self {
            kind: ConditionKind::Expression(expr.into()),
            enabled: true,
            current_hits: 0,
        }
    }

    pub fn hit_count(count: u32) -> Self {
        Self {
            kind: ConditionKind::HitCount(count),
            enabled: true,
            current_hits: 0,
        }
    }

    pub fn log_message(pattern: impl Into<String>) -> Self {
        Self {
            kind: ConditionKind::LogMessage(pattern.into()),
            enabled: true,
            current_hits: 0,
        }
    }

    /// Record a hit and return whether the breakpoint should trigger.
    pub fn record_hit(&mut self) -> bool {
        if !self.enabled {
            return true; // unconditional if disabled
        }
        self.current_hits += 1;
        match &self.kind {
            ConditionKind::HitCount(target) => self.current_hits >= *target,
            ConditionKind::Expression(_) | ConditionKind::LogMessage(_) => true,
        }
    }

    pub fn reset_hits(&mut self) {
        self.current_hits = 0;
    }

    pub fn current_hits(&self) -> u32 {
        self.current_hits
    }

    /// A human-readable description of the condition.
    pub fn description(&self) -> String {
        match &self.kind {
            ConditionKind::Expression(e) => format!("when: {}", e),
            ConditionKind::HitCount(n) => format!("hit count >= {}", n),
            ConditionKind::LogMessage(m) => format!("log: {}", m),
        }
    }
}

// ---------------------------------------------------------------------------
// VariableInspector — deep inspection extensions
// ---------------------------------------------------------------------------

impl VariableInspector {
    /// Expand all top-level variables.
    pub fn expand_all(&mut self) {
        for var in &self.variables {
            let path = var.name.clone();
            if !self.is_expanded(&path) {
                self.expanded_paths.push(path);
            }
        }
    }

    /// Collapse all expanded paths.
    pub fn collapse_all(&mut self) {
        self.expanded_paths.clear();
    }

    /// Number of currently expanded paths.
    pub fn expanded_count(&self) -> usize {
        self.expanded_paths.len()
    }

    /// Search variables (name or value) and return matching paths.
    pub fn search(&self, query: &str) -> Vec<String> {
        let mut results = Vec::new();
        for var in &self.variables {
            self.search_rec(var, &var.name, query, &mut results);
        }
        results
    }

    fn search_rec(&self, var: &DebugVariable, path: &str, query: &str, out: &mut Vec<String>) {
        if var.name.contains(query) || var.value.contains(query) {
            out.push(path.to_string());
        }
        for child in &var.children {
            let child_path = format!("{}.{}", path, child.name);
            self.search_rec(child, &child_path, query, out);
        }
    }

    /// Get the total number of variables (recursive, all depths).
    pub fn total_variable_count(&self) -> usize {
        self.variables.iter().map(|v| Self::count_rec(v)).sum()
    }

    fn count_rec(var: &DebugVariable) -> usize {
        1 + var.children.iter().map(|c| Self::count_rec(c)).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// DebugSessionStateMachine — state machine for debug session lifecycle
// ---------------------------------------------------------------------------

/// Tracks the debug session lifecycle with valid state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// No session active.
    Idle,
    /// Launch/attach request sent, waiting for initialized event.
    Launching,
    /// Session running, program executing.
    Running,
    /// Execution paused at a breakpoint, step, or exception.
    Paused,
    /// Session is terminating.
    Stopping,
}

/// A debug session state machine that enforces valid transitions and tracks
/// step operation history.
#[derive(Debug, Clone)]
pub struct DebugSessionStateMachine {
    phase: SessionPhase,
    /// Running count of step operations performed in this session.
    step_count: u64,
    /// The last step action taken (StepOver, StepIn, StepOut).
    last_step_action: Option<DebugAction>,
    /// Whether an exception breakpoint has been hit.
    exception_hit: bool,
    /// History of phase transitions for diagnostics.
    transition_log: Vec<(SessionPhase, SessionPhase)>,
    max_log_entries: usize,
}

impl DebugSessionStateMachine {
    pub fn new() -> Self {
        Self {
            phase: SessionPhase::Idle,
            step_count: 0,
            last_step_action: None,
            exception_hit: false,
            transition_log: Vec::new(),
            max_log_entries: 100,
        }
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub fn step_count(&self) -> u64 {
        self.step_count
    }

    pub fn last_step_action(&self) -> Option<DebugAction> {
        self.last_step_action
    }

    pub fn exception_hit(&self) -> bool {
        self.exception_hit
    }

    /// Attempt a state transition. Returns `true` if the transition was valid.
    pub fn transition(&mut self, target: SessionPhase) -> bool {
        let valid = match (self.phase, target) {
            (SessionPhase::Idle, SessionPhase::Launching) => true,
            (SessionPhase::Launching, SessionPhase::Running) => true,
            (SessionPhase::Launching, SessionPhase::Stopping) => true,
            (SessionPhase::Running, SessionPhase::Paused) => true,
            (SessionPhase::Running, SessionPhase::Stopping) => true,
            (SessionPhase::Paused, SessionPhase::Running) => true,
            (SessionPhase::Paused, SessionPhase::Stopping) => true,
            (SessionPhase::Stopping, SessionPhase::Idle) => true,
            _ => false,
        };
        if valid {
            let from = self.phase;
            self.phase = target;
            if self.transition_log.len() >= self.max_log_entries {
                self.transition_log.remove(0);
            }
            self.transition_log.push((from, target));
            // Reset exception flag when resuming
            if target == SessionPhase::Running {
                self.exception_hit = false;
            }
        }
        valid
    }

    /// Record a step action (only valid when paused → running).
    pub fn record_step(&mut self, action: DebugAction) -> bool {
        if self.phase != SessionPhase::Paused {
            return false;
        }
        let is_step = matches!(
            action,
            DebugAction::StepOver | DebugAction::StepIn | DebugAction::StepOut
        );
        if !is_step {
            return false;
        }
        self.last_step_action = Some(action);
        self.step_count += 1;
        self.transition(SessionPhase::Running)
    }

    /// Mark that an exception breakpoint has been hit.
    pub fn mark_exception(&mut self) {
        self.exception_hit = true;
    }

    /// Number of recorded transitions.
    pub fn transition_count(&self) -> usize {
        self.transition_log.len()
    }

    /// Reset the state machine to idle.
    pub fn reset(&mut self) {
        self.phase = SessionPhase::Idle;
        self.step_count = 0;
        self.last_step_action = None;
        self.exception_hit = false;
        self.transition_log.clear();
    }

    /// Returns `true` when the session is in an active state (not idle/stopping).
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            SessionPhase::Launching | SessionPhase::Running | SessionPhase::Paused
        )
    }

    /// Returns `true` when step actions are possible (paused).
    pub fn can_step(&self) -> bool {
        self.phase == SessionPhase::Paused
    }
}

impl Default for DebugSessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VariableFormatter — variable value formatting utilities
// ---------------------------------------------------------------------------

/// Utilities for formatting debug variable values for display.
pub struct VariableFormatter;

impl VariableFormatter {
    /// Truncate a value string to `max_len` characters, appending `…` if truncated.
    pub fn truncate_value(value: &str, max_len: usize) -> String {
        if value.len() <= max_len {
            value.to_string()
        } else {
            let mut s = value[..max_len].to_string();
            s.push('…');
            s
        }
    }

    /// Format a variable as `name: type = value`, with optional truncation.
    pub fn format_variable(var: &DebugVariable, max_value_len: Option<usize>) -> String {
        let val = match max_value_len {
            Some(max) => Self::truncate_value(&var.value, max),
            None => var.value.clone(),
        };
        if var.var_type.is_empty() {
            format!("{} = {}", var.name, val)
        } else {
            format!("{}: {} = {}", var.name, var.var_type, val)
        }
    }

    /// Format a numeric value with thousands separators.
    pub fn format_number(value: &str) -> String {
        // Try to parse as integer for formatting
        if let Ok(n) = value.parse::<i64>() {
            let s = n.abs().to_string();
            let mut result = String::new();
            for (i, ch) in s.chars().rev().enumerate() {
                if i > 0 && i % 3 == 0 {
                    result.push('_');
                }
                result.push(ch);
            }
            let formatted: String = result.chars().rev().collect();
            if n < 0 {
                format!("-{}", formatted)
            } else {
                formatted
            }
        } else {
            value.to_string()
        }
    }

    /// Produce a one-line summary for a compound value (struct/array).
    /// E.g., `MyStruct { 3 fields }` or `Vec<i32> [5 items]`.
    pub fn summarize_compound(var: &DebugVariable) -> String {
        let child_count = var.children.len();
        if var.var_type.contains("Vec") || var.var_type.contains('[') {
            format!("{} [{} items]", var.var_type, child_count)
        } else if child_count > 0 {
            format!("{} {{ {} fields }}", var.var_type, child_count)
        } else if var.variables_reference > 0 {
            format!("{} {{ … }}", var.var_type)
        } else {
            var.value.clone()
        }
    }

    /// Format a string value with quotes and escape characters shown.
    pub fn format_string_value(value: &str, max_len: Option<usize>) -> String {
        let inner = value
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
            .replace('\"', "\\\"");
        let quoted = format!("\"{}\"", inner);
        match max_len {
            Some(max) => Self::truncate_value(&quoted, max),
            None => quoted,
        }
    }
}

// ---------------------------------------------------------------------------
// ExceptionBreakpointConfig — exception breakpoint settings
// ---------------------------------------------------------------------------

/// Configures which exceptions cause the debugger to break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionBreakMode {
    /// Never break on this exception category.
    Never,
    /// Break only on unhandled exceptions.
    Unhandled,
    /// Break on all exceptions in this category.
    Always,
}

/// Configuration for exception breakpoints.
#[derive(Debug, Clone)]
pub struct ExceptionBreakpointConfig {
    filters: Vec<ExceptionFilter>,
}

/// A single exception filter (e.g., "All Exceptions", "Uncaught Exceptions").
#[derive(Debug, Clone)]
pub struct ExceptionFilter {
    pub id: String,
    pub label: String,
    pub mode: ExceptionBreakMode,
    pub description: Option<String>,
}

impl ExceptionFilter {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            mode: ExceptionBreakMode::Never,
            description: None,
        }
    }

    pub fn with_mode(mut self, mode: ExceptionBreakMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.mode != ExceptionBreakMode::Never
    }
}

impl ExceptionBreakpointConfig {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Add a new exception filter.
    pub fn add_filter(&mut self, filter: ExceptionFilter) {
        self.filters.push(filter);
    }

    /// Set the mode for a filter by id. Returns `true` if found.
    pub fn set_mode(&mut self, id: &str, mode: ExceptionBreakMode) -> bool {
        if let Some(f) = self.filters.iter_mut().find(|f| f.id == id) {
            f.mode = mode;
            true
        } else {
            false
        }
    }

    /// Get all enabled filter ids (for sending to DAP).
    pub fn enabled_filter_ids(&self) -> Vec<&str> {
        self.filters
            .iter()
            .filter(|f| f.is_enabled())
            .map(|f| f.id.as_str())
            .collect()
    }

    /// Total number of configured filters.
    pub fn filter_count(&self) -> usize {
        self.filters.len()
    }

    /// Number of enabled filters.
    pub fn enabled_count(&self) -> usize {
        self.filters.iter().filter(|f| f.is_enabled()).count()
    }

    /// Get a filter by id.
    pub fn get_filter(&self, id: &str) -> Option<&ExceptionFilter> {
        self.filters.iter().find(|f| f.id == id)
    }

    /// Create a standard config with typical exception filters.
    pub fn with_defaults() -> Self {
        let mut config = Self::new();
        config.add_filter(
            ExceptionFilter::new("all", "All Exceptions")
                .with_description("Break on all thrown exceptions"),
        );
        config.add_filter(
            ExceptionFilter::new("uncaught", "Uncaught Exceptions")
                .with_mode(ExceptionBreakMode::Unhandled)
                .with_description("Break on exceptions not caught by user code"),
        );
        config
    }
}

impl Default for ExceptionBreakpointConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DebugConsoleMessageFormatter — formats debug console output
// ---------------------------------------------------------------------------

/// Formats messages for the debug console panel.
pub struct DebugConsoleMessageFormatter;

impl DebugConsoleMessageFormatter {
    /// Format a log-style message with timestamp prefix.
    /// Produces `"[HH:MM:SS] message"` given hours, minutes, seconds.
    pub fn format_timestamped(hours: u32, minutes: u32, seconds: u32, message: &str) -> String {
        format!("[{:02}:{:02}:{:02}] {}", hours, minutes, seconds, message)
    }

    /// Format an evaluation result for display in the console.
    pub fn format_eval_result(expression: &str, result: &str) -> String {
        format!("> {} → {}", expression, result)
    }

    /// Format an error message with a prefix indicator.
    pub fn format_error(message: &str) -> String {
        format!("⚠ Error: {}", message)
    }

    /// Format a variable assignment for display.
    pub fn format_assignment(name: &str, value: &str) -> String {
        format!("{} = {}", name, value)
    }

    /// Word-wrap a message to a given width, returning wrapped lines.
    pub fn word_wrap(message: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![message.to_string()];
        }
        let mut lines = Vec::new();
        for line in message.lines() {
            if line.len() <= width {
                lines.push(line.to_string());
            } else {
                let mut remaining = line;
                while remaining.len() > width {
                    // Try to break at a space
                    let break_pos = remaining[..width]
                        .rfind(' ')
                        .unwrap_or(width);
                    let (left, right) = remaining.split_at(break_pos);
                    lines.push(left.to_string());
                    remaining = right.trim_start();
                }
                if !remaining.is_empty() {
                    lines.push(remaining.to_string());
                }
            }
        }
        lines
    }
}

// ---------------------------------------------------------------------------
// CallStackFormatter — renders call stack frames as display strings
// ---------------------------------------------------------------------------

/// Formats call stack frames for display in the sidebar.
pub struct CallStackFormatter;

impl CallStackFormatter {
    /// Format a stack frame as a single display line.
    /// E.g. `"main (src/main.rs:42)"`.
    pub fn format_frame(frame: &StackFrame) -> String {
        if frame.source_path.is_empty() {
            format!("{} <no source>", frame.name)
        } else {
            format!("{} ({}:{})", frame.name, frame.source_path, frame.line)
        }
    }

    /// Format a frame with just the file basename for compact display.
    pub fn format_frame_compact(frame: &StackFrame) -> String {
        let basename = frame
            .source_path
            .rsplit('/')
            .next()
            .unwrap_or(&frame.source_path);
        if basename.is_empty() {
            frame.name.clone()
        } else {
            format!("{} ({}:{})", frame.name, basename, frame.line)
        }
    }

    /// Format an entire call stack as numbered lines.
    pub fn format_call_stack(frames: &[StackFrame]) -> Vec<String> {
        frames
            .iter()
            .enumerate()
            .map(|(i, f)| format!("#{} {}", i, Self::format_frame(f)))
            .collect()
    }

    /// Extract just the file basenames from a call stack (for breadcrumb display).
    pub fn frame_basenames(frames: &[StackFrame]) -> Vec<&str> {
        frames
            .iter()
            .map(|f| {
                f.source_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&f.source_path)
            })
            .collect()
    }

    /// Return the "depth" indicator for a frame index (indentation dots).
    pub fn depth_indicator(index: usize) -> String {
        if index == 0 {
            "→".to_string()
        } else {
            format!("{} ", "·".repeat(index.min(4)))
        }
    }
}

// ---------------------------------------------------------------------------
// BreakpointSummary — aggregate breakpoint info
// ---------------------------------------------------------------------------

/// Summarizes breakpoint state across files.
#[derive(Debug, Clone)]
pub struct BreakpointSummary {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub with_condition: usize,
    pub total_hits: u32,
    pub files: Vec<String>,
}

impl BreakpointSummary {
    /// Build a summary from a slice of breakpoints.
    pub fn from_breakpoints(bps: &[Breakpoint]) -> Self {
        let enabled = bps.iter().filter(|b| b.enabled).count();
        let with_condition = bps.iter().filter(|b| b.condition.is_some()).count();
        let total_hits: u32 = bps.iter().map(|b| b.hit_count).sum();
        let mut files: Vec<String> = bps.iter().map(|b| b.file_path.clone()).collect();
        files.sort();
        files.dedup();
        Self {
            total: bps.len(),
            enabled,
            disabled: bps.len() - enabled,
            with_condition,
            total_hits,
            files,
        }
    }

    pub fn file_count(&self) -> usize { self.files.len() }

    pub fn has_conditions(&self) -> bool { self.with_condition > 0 }

    pub fn hit_rate(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.total_hits as f64 / self.total as f64 }
    }
}

impl fmt::Display for BreakpointSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} breakpoints ({} enabled) across {} files",
            self.total, self.enabled, self.files.len()
        )
    }
}

// ---------------------------------------------------------------------------
// CallStackNavigator — navigate the call stack
// ---------------------------------------------------------------------------

/// Navigates up and down a call stack.
#[derive(Debug, Clone)]
pub struct CallStackNavigator {
    frames: Vec<StackFrame>,
    selected: Option<usize>,
}

impl CallStackNavigator {
    pub fn new() -> Self {
        Self { frames: Vec::new(), selected: None }
    }

    pub fn push_frame(&mut self, frame: StackFrame) {
        self.frames.push(frame);
        if self.selected.is_none() {
            self.selected = Some(0);
        }
    }

    pub fn pop_frame(&mut self) -> Option<StackFrame> {
        let f = self.frames.pop();
        if self.frames.is_empty() {
            self.selected = None;
        } else if let Some(sel) = self.selected {
            if sel >= self.frames.len() {
                self.selected = Some(self.frames.len() - 1);
            }
        }
        f
    }

    /// Currently selected frame.
    pub fn current(&self) -> Option<&StackFrame> {
        self.selected.and_then(|i| self.frames.get(i))
    }

    /// Move selection up (toward caller).
    pub fn select_up(&mut self) -> bool {
        match self.selected {
            Some(i) if i + 1 < self.frames.len() => { self.selected = Some(i + 1); true }
            _ => false,
        }
    }

    /// Move selection down (toward callee).
    pub fn select_down(&mut self) -> bool {
        match self.selected {
            Some(i) if i > 0 => { self.selected = Some(i - 1); true }
            _ => false,
        }
    }

    pub fn depth(&self) -> usize { self.frames.len() }

    /// Frames belonging to a specific source file.
    pub fn frames_in_file(&self, path: &str) -> Vec<&StackFrame> {
        self.frames.iter().filter(|f| f.source_path == path).collect()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.selected = None;
    }
}

impl Default for CallStackNavigator {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// VariableWatchItem — watched variable with staleness tracking
// ---------------------------------------------------------------------------

/// A single watched variable or expression in the debug view.
#[derive(Debug, Clone)]
pub struct VariableWatchItem {
    pub name: String,
    pub expression: String,
    pub value: String,
    pub stale: bool,
}

impl VariableWatchItem {
    pub fn new(name: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expression: expression.into(),
            value: String::new(),
            stale: true,
        }
    }

    /// Update the watch with a new value and mark as fresh.
    pub fn update_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.stale = false;
    }

    /// Mark the watch as stale (needs re-evaluation).
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Format for display: "name = value" or "name = <stale>".
    pub fn format_display(&self) -> String {
        if self.stale {
            format!("{} = <stale>", self.name)
        } else {
            format!("{} = {}", self.name, self.value)
        }
    }
}

/// A list of watched variables.
#[derive(Debug, Clone)]
pub struct VariableWatchList {
    watches: Vec<VariableWatchItem>,
}

impl VariableWatchList {
    pub fn new() -> Self { Self { watches: Vec::new() } }

    pub fn add(&mut self, item: VariableWatchItem) {
        self.watches.push(item);
    }

    pub fn remove(&mut self, name: &str) {
        self.watches.retain(|w| w.name != name);
    }

    /// Mark all watches stale (e.g., after a step).
    pub fn mark_all_stale(&mut self) {
        for w in &mut self.watches {
            w.mark_stale();
        }
    }

    pub fn get(&self, name: &str) -> Option<&VariableWatchItem> {
        self.watches.iter().find(|w| w.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut VariableWatchItem> {
        self.watches.iter_mut().find(|w| w.name == name)
    }

    pub fn stale_count(&self) -> usize {
        self.watches.iter().filter(|w| w.stale).count()
    }

    pub fn len(&self) -> usize { self.watches.len() }
    pub fn is_empty(&self) -> bool { self.watches.is_empty() }
}

impl Default for VariableWatchList {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for debug_view functionality.
pub struct DebugViewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl DebugViewConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &DebugViewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for debug_view operations.
pub struct DebugViewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl DebugViewRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for debug_view.
pub struct DebugViewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl DebugViewValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &DebugViewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Debug session UI widgets — extended utilities (qe)
// ---------------------------------------------------------------------------

/// Metric accumulator for dbg_view operations.
#[derive(Debug, Clone)]
pub struct QeMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QeMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for dbg_view.
#[derive(Debug, Clone)]
pub struct QeRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QeRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for dbg_view lookups.
#[derive(Debug, Clone)]
pub struct QeLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QeLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for debug_view
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaDebugViewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaDebugViewRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaDebugViewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaDebugViewCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaDebugViewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 25
// ---------------------------------------------------------------------------

/// Generic object pool `Xc25Pool<T>`.
pub struct Xc25Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc25Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc25PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc25Pool<T> {
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
    pub fn stats(&self) -> Xc25PoolStats {
        Xc25PoolStats {
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

impl<T> Default for Xc25Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc25Scheduler`.
pub struct Xc25Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc25Scheduler {
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

impl Default for Xc25Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_25 hash for the given byte slice.
pub fn xc_25_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_25 convention.
pub fn xc_25_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_119 deepening: state machine + event bus ---

/// States for the Xd119 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd119State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd119State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd119Transition {
    pub from: Xd119State,
    pub to: Xd119State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd119StateMachine {
    current: Xd119State,
    history: Vec<Xd119Transition>,
    step_counter: usize,
}

impl Xd119StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd119State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd119State {
        self.current
    }

    pub fn history(&self) -> &[Xd119Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd119State) -> Result<Xd119State, String> {
        let allowed = match (self.current, target) {
            (Xd119State::Idle, Xd119State::Running) => true,
            (Xd119State::Running, Xd119State::Paused) => true,
            (Xd119State::Running, Xd119State::Done) => true,
            (Xd119State::Paused, Xd119State::Running) => true,
            (Xd119State::Paused, Xd119State::Done) => true,
            (Xd119State::Done, Xd119State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_119: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd119Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd119SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd119State> {
        let prefix = "Xd119SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd119State::Idle),
            "Running" => Some(Xd119State::Running),
            "Paused" => Some(Xd119State::Paused),
            "Done" => Some(Xd119State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd119State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd119 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd119Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd119Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd119HandlerFn = Box<dyn Fn(&Xd119Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd119EventBus {
    handlers: Vec<(usize, Option<String>, Xd119HandlerFn)>,
    next_id: usize,
    published: Vec<Xd119Event>,
}

impl Xd119EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd119Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd119Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd119Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd119Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_46: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg46Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg46Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg46Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_46: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg46Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg46Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg46Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg46Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 24).
pub struct Xh24SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh24SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 66 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 24).
pub struct Xh24BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh24BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 24).
pub struct Xi24Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi24Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi24Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi24Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 24).
pub struct Xi24IntervalTree {
    xi_intervals: Vec<Xi24Interval>,
}

impl Xi24IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi24Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi24Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi24Interval) -> Vec<&Xi24Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi24Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi24Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi24Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi24Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi24Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi24Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 25) ---

/// Disjoint set / union-find for crate 25.
pub struct Xj25UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj25UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ25_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 25.
pub struct Xj25BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj25BTreeNode<K, V>>>,
    len: usize,
}

struct Xj25BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj25BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj25BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ25_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ25_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj25BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj25BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj25BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj25BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_25 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk25SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk25SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk25DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk25DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_25).
#[derive(Debug, Clone)]
pub struct Xl25Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl25Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_25).
#[derive(Debug, Clone)]
pub struct Xl25SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl25SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm25MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm25MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm25Tokenizer {
    text: String,
}

impl Xm25Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 24.
pub struct Xn24Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn24Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 24 -----

#[derive(Debug, Clone)]
struct Xn24AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn24AvlNode<K, V>>>,
    right: Option<Box<Xn24AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 24.
#[derive(Debug, Clone)]
pub struct Xn24AVL<K, V> {
    root: Option<Box<Xn24AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn24AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn24AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn24AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn24AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn24AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn24AvlNode<K, V>>) -> Box<Xn24AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn24AvlNode<K, V>>) -> Box<Xn24AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn24AvlNode<K, V>>) -> Box<Xn24AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn24AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn24AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn24AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn24AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn24AvlNode<K, V>>) -> &Xn24AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn24AvlNode<K, V>>) -> (Box<Xn24AvlNode<K, V>>, Option<Box<Xn24AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn24AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn24AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn24AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn24AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn24AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn24AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn24AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
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

    // ── BreakpointManager tests ──

    #[test]
    fn bp_manager_add_and_count() {
        let mut mgr = BreakpointManager::new();
        let id1 = mgr.add("src/main.rs", 10);
        let id2 = mgr.add("src/main.rs", 20);
        assert_eq!(mgr.count(), 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn bp_manager_remove() {
        let mut mgr = BreakpointManager::new();
        let id = mgr.add("test.rs", 5);
        assert!(mgr.remove(id));
        assert!(!mgr.remove(id));
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn bp_manager_toggle_enabled() {
        let mut mgr = BreakpointManager::new();
        let id = mgr.add("test.rs", 1);
        assert_eq!(mgr.count_enabled(), 1);
        mgr.toggle_enabled(id);
        assert_eq!(mgr.count_enabled(), 0);
        mgr.toggle_enabled(id);
        assert_eq!(mgr.count_enabled(), 1);
        assert!(!mgr.toggle_enabled(999));
    }

    #[test]
    fn bp_manager_find_by_file() {
        let mut mgr = BreakpointManager::new();
        mgr.add("a.rs", 1);
        mgr.add("a.rs", 5);
        mgr.add("b.rs", 10);
        assert_eq!(mgr.find_by_file("a.rs").len(), 2);
        assert_eq!(mgr.find_by_file("b.rs").len(), 1);
        assert!(mgr.find_by_file("c.rs").is_empty());
    }

    #[test]
    fn bp_manager_find_by_file_line() {
        let mut mgr = BreakpointManager::new();
        mgr.add("a.rs", 42);
        assert!(mgr.find_by_file_line("a.rs", 42).is_some());
        assert!(mgr.find_by_file_line("a.rs", 99).is_none());
    }

    // -----------------------------------------------------------------------
    // CallStackFrameInfo tests
    // -----------------------------------------------------------------------

    #[test]
    fn call_stack_frame_info_display_location() {
        let mut frame = CallStackFrameInfo::new(1, "main", 42, 1);
        assert_eq!(frame.display_location(), "<unknown>");

        frame.source_file = Some("main.rs".into());
        assert_eq!(frame.display_location(), "main.rs:42:1");
    }

    #[test]
    fn call_stack_frame_info_short_name() {
        let frame = CallStackFrameInfo::new(1, "mymod::inner::run", 1, 0);
        assert_eq!(frame.short_name(), "run");

        let simple = CallStackFrameInfo::new(2, "main", 1, 0);
        assert_eq!(simple.short_name(), "main");
    }

    #[test]
    fn call_stack_frame_info_is_user_code() {
        let mut frame = CallStackFrameInfo::new(1, "f", 1, 0);
        assert!(frame.is_user_code());

        frame.is_external = true;
        assert!(!frame.is_user_code());
    }

    // -----------------------------------------------------------------------
    // VariableInspector tests
    // -----------------------------------------------------------------------

    #[test]
    fn variable_inspector_toggle_and_flatten() {
        let child = DebugVariable::new("x", "42", "i32");
        let parent = DebugVariable::new("obj", "{...}", "MyStruct")
            .with_children(vec![child]);

        let mut inspector = VariableInspector::new();
        inspector.set_variables(vec![parent]);
        assert_eq!(inspector.variable_count(), 1);

        // Collapsed: only root visible
        let flat = inspector.flatten();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].0, 0);
        assert_eq!(flat[0].1.name, "obj");

        // Expand root
        inspector.toggle_path("obj");
        assert!(inspector.is_expanded("obj"));
        let flat = inspector.flatten();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[1].0, 1);
        assert_eq!(flat[1].1.name, "x");

        // Collapse again
        inspector.toggle_path("obj");
        assert!(!inspector.is_expanded("obj"));
        assert_eq!(inspector.flatten().len(), 1);
    }

    #[test]
    fn variable_inspector_nested_expand() {
        let grandchild = DebugVariable::new("z", "true", "bool");
        let child = DebugVariable::new("inner", "{}", "Sub").with_children(vec![grandchild]);
        let root = DebugVariable::new("root", "{}", "Top").with_children(vec![child]);

        let mut inspector = VariableInspector::new();
        inspector.set_variables(vec![root]);

        inspector.toggle_path("root");
        inspector.toggle_path("root.inner");
        let flat = inspector.flatten();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].0, 0); // root  depth 0
        assert_eq!(flat[1].0, 1); // inner depth 1
        assert_eq!(flat[2].0, 2); // z     depth 2
    }

    // -----------------------------------------------------------------------
    // BreakpointList tests
    // -----------------------------------------------------------------------

    #[test]
    fn breakpoint_list_add_select_remove() {
        let mut list = BreakpointList::new();
        assert!(list.selected_breakpoint().is_none());

        list.add(Breakpoint::new(1, "a.rs", 10));
        list.add(Breakpoint::new(2, "b.rs", 20));

        assert_eq!(list.selected_breakpoint().unwrap().id, 1);
        list.select_next();
        assert_eq!(list.selected_breakpoint().unwrap().id, 2);
        list.select_previous();
        assert_eq!(list.selected_breakpoint().unwrap().id, 1);

        assert!(list.remove(1));
        assert_eq!(list.selected_breakpoint().unwrap().id, 2);
        assert!(!list.remove(999));
    }

    #[test]
    fn breakpoint_list_toggle_enable_disable() {
        let mut list = BreakpointList::new();
        list.add(Breakpoint::new(1, "a.rs", 1));
        list.add(Breakpoint::new(2, "b.rs", 2));
        assert_eq!(list.enabled_count(), 2);

        list.toggle(1);
        assert_eq!(list.enabled_count(), 1);

        list.disable_all();
        assert_eq!(list.enabled_count(), 0);

        list.enable_all();
        assert_eq!(list.enabled_count(), 2);

        assert!(!list.toggle(999));
    }

    #[test]
    fn breakpoint_list_for_file() {
        let mut list = BreakpointList::new();
        list.add(Breakpoint::new(1, "a.rs", 1));
        list.add(Breakpoint::new(2, "a.rs", 5));
        list.add(Breakpoint::new(3, "b.rs", 10));

        assert_eq!(list.breakpoints_for_file("a.rs").len(), 2);
        assert_eq!(list.breakpoints_for_file("b.rs").len(), 1);
        assert!(list.breakpoints_for_file("c.rs").is_empty());
    }

    // --- New tests ---

    #[test]
    fn watch_expression_lifecycle() {
        let mut w = WatchExpression::new("x + 1");
        assert!(w.is_pending());
        assert_eq!(w.display_text(), "x + 1 …");

        w.set_result("42");
        assert!(!w.is_pending());
        assert!(!w.is_error());
        assert_eq!(w.display_text(), "x + 1 = 42");
        assert_eq!(w.eval_count, 1);

        w.set_error("undefined");
        assert!(w.is_error());
        assert_eq!(w.display_text(), "x + 1 ⚠ undefined");
        assert_eq!(w.eval_count, 2);
    }

    #[test]
    fn watch_expression_list_operations() {
        let mut list = WatchExpressionList::new();
        list.add("a");
        list.add("b");
        assert_eq!(list.len(), 2);

        list.get_mut(0).unwrap().set_error("fail");
        assert_eq!(list.error_count(), 1);

        list.reset_all();
        assert!(list.get(0).unwrap().is_pending());
        assert_eq!(list.error_count(), 0);

        assert!(list.remove(0));
        assert_eq!(list.len(), 1);
        assert!(!list.remove(99));
    }

    #[test]
    fn debug_console_history_navigation() {
        let mut h = DebugConsoleHistory::new(5);
        h.push("print x");
        h.push("print y");
        h.push("step");

        assert_eq!(h.up(), Some("step"));
        assert_eq!(h.up(), Some("print y"));
        assert_eq!(h.up(), Some("print x"));
        assert_eq!(h.up(), Some("print x")); // stays at start

        assert_eq!(h.down(), Some("print y"));
        assert_eq!(h.down(), Some("step"));
        assert_eq!(h.down(), None); // past end
    }

    #[test]
    fn debug_console_history_dedup_and_max() {
        let mut h = DebugConsoleHistory::new(3);
        h.push("a");
        h.push("a"); // consecutive duplicate
        assert_eq!(h.len(), 1);

        h.push("b");
        h.push("c");
        h.push("d"); // exceeds max, drops oldest
        assert_eq!(h.len(), 3);
        assert_eq!(h.search("a").len(), 0); // "a" was evicted
    }

    #[test]
    fn debug_console_history_search() {
        let mut h = DebugConsoleHistory::new(10);
        h.push("print x");
        h.push("step over");
        h.push("print y");
        let results = h.search("print");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn breakpoint_condition_hit_count() {
        let mut cond = BreakpointCondition::hit_count(3);
        assert_eq!(cond.description(), "hit count >= 3");
        assert!(!cond.record_hit()); // 1
        assert!(!cond.record_hit()); // 2
        assert!(cond.record_hit());  // 3
        assert_eq!(cond.current_hits(), 3);
        cond.reset_hits();
        assert_eq!(cond.current_hits(), 0);
    }

    #[test]
    fn breakpoint_condition_expression() {
        let mut cond = BreakpointCondition::expression("i > 5");
        assert_eq!(cond.description(), "when: i > 5");
        assert!(cond.record_hit()); // expressions always trigger
    }

    #[test]
    fn variable_inspector_expand_collapse_all() {
        let mut inspector = VariableInspector::new();
        let child = DebugVariable::new("x", "1", "i32");
        let parent = DebugVariable::new("obj", "{}", "Struct").with_children(vec![child]);
        inspector.set_variables(vec![parent]);

        inspector.expand_all();
        assert_eq!(inspector.expanded_count(), 1);
        assert_eq!(inspector.flatten().len(), 2); // parent + child

        inspector.collapse_all();
        assert_eq!(inspector.expanded_count(), 0);
        assert_eq!(inspector.flatten().len(), 1); // parent only
    }

    #[test]
    fn variable_inspector_search_and_count() {
        let mut inspector = VariableInspector::new();
        let child = DebugVariable::new("count", "42", "i32");
        let parent = DebugVariable::new("stats", "{}", "Stats").with_children(vec![child]);
        inspector.set_variables(vec![parent, DebugVariable::new("name", "hello", "String")]);

        let results = inspector.search("count");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "stats.count");

        assert_eq!(inspector.total_variable_count(), 3); // stats + count + name
    }

    // -----------------------------------------------------------------------
    // DebugSessionStateMachine tests
    // -----------------------------------------------------------------------

    #[test]
    fn session_state_machine_valid_transitions() {
        let mut sm = DebugSessionStateMachine::new();
        assert_eq!(sm.phase(), SessionPhase::Idle);
        assert!(!sm.is_active());

        assert!(sm.transition(SessionPhase::Launching));
        assert!(sm.is_active());
        assert!(sm.transition(SessionPhase::Running));
        assert!(sm.transition(SessionPhase::Paused));
        assert!(sm.can_step());
        assert!(sm.transition(SessionPhase::Running));
        assert!(!sm.can_step());
        assert!(sm.transition(SessionPhase::Stopping));
        assert!(!sm.is_active());
        assert!(sm.transition(SessionPhase::Idle));

        assert_eq!(sm.transition_count(), 6);
    }

    #[test]
    fn session_state_machine_invalid_transitions() {
        let mut sm = DebugSessionStateMachine::new();
        // Can't go from Idle directly to Running
        assert!(!sm.transition(SessionPhase::Running));
        assert_eq!(sm.phase(), SessionPhase::Idle);
        // Can't go from Idle to Paused
        assert!(!sm.transition(SessionPhase::Paused));
    }

    #[test]
    fn session_state_machine_step_tracking() {
        let mut sm = DebugSessionStateMachine::new();
        sm.transition(SessionPhase::Launching);
        sm.transition(SessionPhase::Running);
        sm.transition(SessionPhase::Paused);

        assert!(sm.record_step(DebugAction::StepOver));
        assert_eq!(sm.step_count(), 1);
        assert_eq!(sm.last_step_action(), Some(DebugAction::StepOver));
        // After step, phase should be Running
        assert_eq!(sm.phase(), SessionPhase::Running);

        // Can't step when running
        assert!(!sm.record_step(DebugAction::StepIn));
        assert_eq!(sm.step_count(), 1);

        // Continue is not a step action
        sm.transition(SessionPhase::Paused);
        assert!(!sm.record_step(DebugAction::Continue));
    }

    #[test]
    fn session_state_machine_exception_and_reset() {
        let mut sm = DebugSessionStateMachine::new();
        sm.transition(SessionPhase::Launching);
        sm.transition(SessionPhase::Running);
        sm.transition(SessionPhase::Paused);
        sm.mark_exception();
        assert!(sm.exception_hit());

        // Resuming clears exception flag
        sm.transition(SessionPhase::Running);
        assert!(!sm.exception_hit());

        sm.reset();
        assert_eq!(sm.phase(), SessionPhase::Idle);
        assert_eq!(sm.step_count(), 0);
        assert_eq!(sm.transition_count(), 0);
    }

    // -----------------------------------------------------------------------
    // VariableFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn variable_formatter_truncate() {
        assert_eq!(VariableFormatter::truncate_value("hello", 10), "hello");
        assert_eq!(VariableFormatter::truncate_value("hello world", 5), "hello…");
        assert_eq!(VariableFormatter::truncate_value("", 5), "");
    }

    #[test]
    fn variable_formatter_format_variable() {
        let var = DebugVariable::new("count", "42", "i32");
        assert_eq!(
            VariableFormatter::format_variable(&var, None),
            "count: i32 = 42"
        );
        assert_eq!(
            VariableFormatter::format_variable(&var, Some(1)),
            "count: i32 = 4…"
        );

        let untyped = DebugVariable::new("x", "hello", "");
        assert_eq!(
            VariableFormatter::format_variable(&untyped, None),
            "x = hello"
        );
    }

    #[test]
    fn variable_formatter_format_number() {
        assert_eq!(VariableFormatter::format_number("1234567"), "1_234_567");
        assert_eq!(VariableFormatter::format_number("-42000"), "-42_000");
        assert_eq!(VariableFormatter::format_number("42"), "42");
        assert_eq!(VariableFormatter::format_number("abc"), "abc");
    }

    #[test]
    fn variable_formatter_summarize_compound() {
        let child = DebugVariable::new("x", "1", "i32");
        let vec_var = DebugVariable::new("items", "[...]", "Vec<i32>")
            .with_children(vec![child.clone(), child.clone()]);
        assert_eq!(
            VariableFormatter::summarize_compound(&vec_var),
            "Vec<i32> [2 items]"
        );

        let struct_var = DebugVariable::new("obj", "{}", "MyStruct")
            .with_children(vec![child]);
        assert_eq!(
            VariableFormatter::summarize_compound(&struct_var),
            "MyStruct { 1 fields }"
        );

        let mut lazy = DebugVariable::new("ref", "{}", "Unknown");
        lazy.variables_reference = 5;
        assert_eq!(
            VariableFormatter::summarize_compound(&lazy),
            "Unknown { … }"
        );
    }

    #[test]
    fn variable_formatter_format_string_value() {
        assert_eq!(
            VariableFormatter::format_string_value("hello", None),
            "\"hello\""
        );
        assert_eq!(
            VariableFormatter::format_string_value("line\nnew", None),
            "\"line\\nnew\""
        );
        assert_eq!(
            VariableFormatter::format_string_value("long string", Some(8)),
            "\"long st…"
        );
    }

    // -----------------------------------------------------------------------
    // ExceptionBreakpointConfig tests
    // -----------------------------------------------------------------------

    #[test]
    fn exception_breakpoint_config_defaults() {
        let config = ExceptionBreakpointConfig::with_defaults();
        assert_eq!(config.filter_count(), 2);
        // "all" is disabled by default, "uncaught" is Unhandled
        assert_eq!(config.enabled_count(), 1);
        let ids = config.enabled_filter_ids();
        assert_eq!(ids, vec!["uncaught"]);
    }

    #[test]
    fn exception_breakpoint_config_set_mode() {
        let mut config = ExceptionBreakpointConfig::with_defaults();
        assert!(config.set_mode("all", ExceptionBreakMode::Always));
        assert_eq!(config.enabled_count(), 2);
        assert!(!config.set_mode("nonexistent", ExceptionBreakMode::Always));

        let filter = config.get_filter("all").unwrap();
        assert!(filter.is_enabled());
        assert_eq!(filter.mode, ExceptionBreakMode::Always);
        assert!(filter.description.is_some());
    }

    // -----------------------------------------------------------------------
    // DebugConsoleMessageFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn console_formatter_timestamped() {
        assert_eq!(
            DebugConsoleMessageFormatter::format_timestamped(14, 5, 9, "Hello"),
            "[14:05:09] Hello"
        );
    }

    #[test]
    fn console_formatter_eval_and_error() {
        assert_eq!(
            DebugConsoleMessageFormatter::format_eval_result("x + 1", "42"),
            "> x + 1 → 42"
        );
        assert_eq!(
            DebugConsoleMessageFormatter::format_error("segfault"),
            "⚠ Error: segfault"
        );
        assert_eq!(
            DebugConsoleMessageFormatter::format_assignment("x", "10"),
            "x = 10"
        );
    }

    #[test]
    fn console_formatter_word_wrap() {
        let lines = DebugConsoleMessageFormatter::word_wrap("short", 80);
        assert_eq!(lines, vec!["short"]);

        let lines = DebugConsoleMessageFormatter::word_wrap("hello world foo", 11);
        assert_eq!(lines, vec!["hello", "world foo"]);

        let lines = DebugConsoleMessageFormatter::word_wrap("hello world foo bar", 20);
        assert_eq!(lines, vec!["hello world foo bar"]);

        let lines = DebugConsoleMessageFormatter::word_wrap("abcdef", 3);
        assert_eq!(lines, vec!["abc", "def"]);

        // Zero width returns original
        let lines = DebugConsoleMessageFormatter::word_wrap("test", 0);
        assert_eq!(lines, vec!["test"]);
    }

    // -----------------------------------------------------------------------
    // CallStackFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn call_stack_formatter_format_frame() {
        let frame = StackFrame::new(1, "main", "src/main.rs", 42, 1);
        assert_eq!(
            CallStackFormatter::format_frame(&frame),
            "main (src/main.rs:42)"
        );

        let no_source = StackFrame::new(2, "unknown_fn", "", 0, 0);
        assert_eq!(
            CallStackFormatter::format_frame(&no_source),
            "unknown_fn <no source>"
        );
    }

    #[test]
    fn call_stack_formatter_compact() {
        let frame = StackFrame::new(1, "run", "/long/path/to/main.rs", 10, 1);
        assert_eq!(
            CallStackFormatter::format_frame_compact(&frame),
            "run (main.rs:10)"
        );
    }

    #[test]
    fn call_stack_formatter_format_stack() {
        let frames = vec![
            StackFrame::new(1, "inner", "a.rs", 5, 1),
            StackFrame::new(2, "outer", "b.rs", 10, 1),
        ];
        let formatted = CallStackFormatter::format_call_stack(&frames);
        assert_eq!(formatted.len(), 2);
        assert_eq!(formatted[0], "#0 inner (a.rs:5)");
        assert_eq!(formatted[1], "#1 outer (b.rs:10)");
    }

    #[test]
    fn call_stack_formatter_basenames_and_depth() {
        let frames = vec![
            StackFrame::new(1, "f", "/a/b/main.rs", 1, 1),
            StackFrame::new(2, "g", "/x/lib.rs", 2, 1),
        ];
        let basenames = CallStackFormatter::frame_basenames(&frames);
        assert_eq!(basenames, vec!["main.rs", "lib.rs"]);

        assert_eq!(CallStackFormatter::depth_indicator(0), "→");
        assert!(CallStackFormatter::depth_indicator(2).starts_with("··"));
    }

    // -- BreakpointSummary ---------------------------------------------------

    #[test]
    fn breakpoint_summary_basic() {
        let bps = vec![
            Breakpoint::new(1, "main.rs", 10),
            Breakpoint { id: 2, file_path: "main.rs".into(), line: 20, enabled: false, condition: None, hit_count: 0 },
            Breakpoint { id: 3, file_path: "lib.rs".into(), line: 5, enabled: true, condition: Some("x > 0".into()), hit_count: 3 },
        ];
        let s = BreakpointSummary::from_breakpoints(&bps);
        assert_eq!(s.total, 3);
        assert_eq!(s.enabled, 2);
        assert_eq!(s.disabled, 1);
        assert_eq!(s.file_count(), 2);
        assert!(s.has_conditions());
    }

    #[test]
    fn breakpoint_summary_empty() {
        let s = BreakpointSummary::from_breakpoints(&[]);
        assert_eq!(s.total, 0);
        assert!(!s.has_conditions());
        assert_eq!(s.hit_rate(), 0.0);
    }

    #[test]
    fn breakpoint_summary_display() {
        let bps = vec![Breakpoint::new(1, "a.rs", 1)];
        let s = BreakpointSummary::from_breakpoints(&bps);
        let d = format!("{s}");
        assert!(d.contains("1 breakpoints"));
    }

    // -- CallStackNavigator ---------------------------------------------------

    #[test]
    fn navigator_push_and_current() {
        let mut nav = CallStackNavigator::new();
        nav.push_frame(StackFrame::new(1, "main", "main.rs", 10, 1));
        assert_eq!(nav.depth(), 1);
        assert_eq!(nav.current().unwrap().name, "main");
    }

    #[test]
    fn navigator_up_down() {
        let mut nav = CallStackNavigator::new();
        nav.push_frame(StackFrame::new(1, "inner", "a.rs", 1, 1));
        nav.push_frame(StackFrame::new(2, "outer", "b.rs", 1, 1));
        assert!(nav.select_up());
        assert_eq!(nav.current().unwrap().name, "outer");
        assert!(nav.select_down());
        assert_eq!(nav.current().unwrap().name, "inner");
    }

    #[test]
    fn navigator_pop() {
        let mut nav = CallStackNavigator::new();
        nav.push_frame(StackFrame::new(1, "a", "x.rs", 1, 1));
        nav.push_frame(StackFrame::new(2, "b", "y.rs", 1, 1));
        let f = nav.pop_frame().unwrap();
        assert_eq!(f.name, "b");
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn navigator_frames_in_file() {
        let mut nav = CallStackNavigator::new();
        nav.push_frame(StackFrame::new(1, "a", "main.rs", 1, 1));
        nav.push_frame(StackFrame::new(2, "b", "lib.rs", 1, 1));
        nav.push_frame(StackFrame::new(3, "c", "main.rs", 5, 1));
        assert_eq!(nav.frames_in_file("main.rs").len(), 2);
    }

    // -- VariableWatchItem / VariableWatchList ---------------------------------

    #[test]
    fn watch_item_update() {
        let mut w = VariableWatchItem::new("x", "x + 1");
        assert!(w.stale);
        w.update_value("42");
        assert!(!w.stale);
        assert_eq!(w.format_display(), "x = 42");
    }

    #[test]
    fn watch_item_stale_display() {
        let w = VariableWatchItem::new("y", "y");
        assert_eq!(w.format_display(), "y = <stale>");
    }

    #[test]
    fn watch_list_operations() {
        let mut list = VariableWatchList::new();
        list.add(VariableWatchItem::new("a", "a"));
        list.add(VariableWatchItem::new("b", "b"));
        assert_eq!(list.len(), 2);
        assert_eq!(list.stale_count(), 2);
        list.get_mut("a").unwrap().update_value("1");
        assert_eq!(list.stale_count(), 1);
        list.remove("b");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn watch_list_mark_all_stale() {
        let mut list = VariableWatchList::new();
        let mut w = VariableWatchItem::new("x", "x");
        w.update_value("5");
        list.add(w);
        assert_eq!(list.stale_count(), 0);
        list.mark_all_stale();
        assert_eq!(list.stale_count(), 1);
    }

    #[test]
    fn debug_view_config_new() {
        let cfg = DebugViewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn debug_view_config_set_get() {
        let mut cfg = DebugViewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn debug_view_config_remove() {
        let mut cfg = DebugViewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn debug_view_config_keys_sorted() {
        let mut cfg = DebugViewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn debug_view_config_bump_version() {
        let mut cfg = DebugViewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn debug_view_config_clear() {
        let mut cfg = DebugViewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn debug_view_config_merge() {
        let mut cfg1 = DebugViewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = DebugViewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn debug_view_config_disable() {
        let mut cfg = DebugViewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn debug_view_rate_tracker_empty() {
        let rt = DebugViewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn debug_view_rate_tracker_record() {
        let mut rt = DebugViewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn debug_view_rate_tracker_prune() {
        let mut rt = DebugViewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn debug_view_validator_valid() {
        let v = DebugViewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn debug_view_validator_errors() {
        let mut v = DebugViewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn debug_view_validator_clear() {
        let mut v = DebugViewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn debug_view_validator_merge() {
        let mut v1 = DebugViewValidator::new();
        v1.add_error("e1");
        let mut v2 = DebugViewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn debug_view_rate_tracker_clear() {
        let mut rt = DebugViewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qe_metrics_empty() {
        let m = QeMetrics::new("dbg_view");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qe_metrics_record_and_mean() {
        let mut m = QeMetrics::new("dbg_view");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qe_metrics_min_max() {
        let mut m = QeMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qe_metrics_variance_and_std() {
        let mut m = QeMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qe_metrics_percentile() {
        let mut m = QeMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qe_metrics_merge() {
        let mut a = QeMetrics::new("a");
        a.record(1.0);
        let mut b = QeMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qe_metrics_reset() {
        let mut m = QeMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qe_rate_window_empty() {
        let rw = QeRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qe_rate_window_tick_and_rate() {
        let mut rw = QeRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qe_lru_cache_basic() {
        let mut c = QeLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qe_lru_cache_contains_and_keys() {
        let mut c = QeLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qe_lru_cache_remove() {
        let mut c = QeLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qe_metrics_sum() {
        let mut m = QeMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qe_metrics_label() {
        let m = QeMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qe_lru_cache_clear() {
        let mut c = QeLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for debug_view
    #[test]
    fn xa_debug_view_ring_new() {
        let rb = super::XaDebugViewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_debug_view_ring_push_len() {
        let mut rb = super::XaDebugViewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_debug_view_ring_wrap() {
        let mut rb = super::XaDebugViewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_debug_view_ring_mean_empty() {
        let rb = super::XaDebugViewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_debug_view_ring_mean_values() {
        let mut rb = super::XaDebugViewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_debug_view_ring_min_max() {
        let mut rb = super::XaDebugViewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_debug_view_ring_iter() {
        let mut rb = super::XaDebugViewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_debug_view_counter_new() {
        let c = super::XaDebugViewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_debug_view_counter_inc() {
        let mut c = super::XaDebugViewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_debug_view_counter_inc_by() {
        let mut c = super::XaDebugViewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_debug_view_counter_reset() {
        let mut c = super::XaDebugViewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_debug_view_counter_clear() {
        let mut c = super::XaDebugViewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_debug_view_counter_default() {
        let c = super::XaDebugViewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 25 ----

    #[test]
    fn xc_25_pool_new_empty() {
        let pool: super::Xc25Pool<i32> = super::Xc25Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_25_pool_release_acquire() {
        let mut pool = super::Xc25Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_25_pool_acquire_empty() {
        let mut pool: super::Xc25Pool<i32> = super::Xc25Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_25_pool_full() {
        let mut pool = super::Xc25Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_25_pool_drain() {
        let mut pool = super::Xc25Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_25_pool_stats() {
        let mut pool = super::Xc25Pool::new(8);
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
    fn xc_25_pool_clear() {
        let mut pool = super::Xc25Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_25_pool_shrink() {
        let mut pool = super::Xc25Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_25_pool_default() {
        let pool: super::Xc25Pool<String> = super::Xc25Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_25_pool_extend() {
        let mut pool = super::Xc25Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_25_pool_retain() {
        let mut pool = super::Xc25Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_25_scheduler_round_robin() {
        let mut sched = super::Xc25Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_25_scheduler_empty() {
        let mut sched = super::Xc25Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_25_scheduler_reset() {
        let mut sched = super::Xc25Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_25_scheduler_add_remove() {
        let mut sched = super::Xc25Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_25_scheduler_targets() {
        let sched = super::Xc25Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_25_hash_empty() {
        assert_eq!(super::xc_25_hash(b""), 5381);
    }

    #[test]
    fn xc_25_hash_data() {
        let h = super::xc_25_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_25_hash(b"hello"), h);
    }

    #[test]
    fn xc_25_reverse_str() {
        assert_eq!(super::xc_25_reverse("abc"), "cba");
        assert_eq!(super::xc_25_reverse(""), "");
    }


    // --- xd_119 deepening tests ---

    #[test]
    fn xd_119_sm_initial_state() {
        let sm = Xd119StateMachine::new();
        assert_eq!(sm.current_state(), Xd119State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_119_sm_valid_idle_to_running() {
        let mut sm = Xd119StateMachine::new();
        assert!(sm.transition(Xd119State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd119State::Running);
    }

    #[test]
    fn xd_119_sm_valid_running_to_paused() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        assert!(sm.transition(Xd119State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd119State::Paused);
    }

    #[test]
    fn xd_119_sm_valid_running_to_done() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        assert!(sm.transition(Xd119State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd119State::Done);
    }

    #[test]
    fn xd_119_sm_valid_paused_to_running() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        sm.transition(Xd119State::Paused).unwrap();
        assert!(sm.transition(Xd119State::Running).is_ok());
    }

    #[test]
    fn xd_119_sm_valid_done_to_idle() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        sm.transition(Xd119State::Done).unwrap();
        assert!(sm.transition(Xd119State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd119State::Idle);
    }

    #[test]
    fn xd_119_sm_invalid_idle_to_done() {
        let mut sm = Xd119StateMachine::new();
        assert!(sm.transition(Xd119State::Done).is_err());
    }

    #[test]
    fn xd_119_sm_invalid_idle_to_paused() {
        let mut sm = Xd119StateMachine::new();
        assert!(sm.transition(Xd119State::Paused).is_err());
    }

    #[test]
    fn xd_119_sm_history_tracking() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        sm.transition(Xd119State::Paused).unwrap();
        sm.transition(Xd119State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd119State::Idle);
        assert_eq!(sm.history()[0].to, Xd119State::Running);
        assert_eq!(sm.history()[1].from, Xd119State::Running);
        assert_eq!(sm.history()[2].to, Xd119State::Done);
    }

    #[test]
    fn xd_119_sm_serialize_deserialize() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd119StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd119State::Running));
    }

    #[test]
    fn xd_119_sm_deserialize_invalid() {
        assert_eq!(Xd119StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_119_sm_reset() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd119State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_119_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd119EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd119Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_119_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd119EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd119Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd119Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_119_bus_unsubscribe() {
        let mut bus = Xd119EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_119_event_kind_and_payload() {
        let e = Xd119Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd119Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_119_bus_clear_history() {
        let mut bus = Xd119EventBus::new();
        bus.publish(Xd119Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_119_sm_step_counter_increments() {
        let mut sm = Xd119StateMachine::new();
        sm.transition(Xd119State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd119State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_46 graph tests ------------------------------------------------

    #[test]
    fn xg_46_graph_empty() {
        let g = super::Xg46Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_46_graph_add_node() {
        let mut g = super::Xg46Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_46_graph_add_edge() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_46_graph_neighbors() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_46_graph_has_path() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_46_graph_self_path() {
        let g = super::Xg46Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_46_graph_topo_sort() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_46_graph_cycle_detect_false() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_46_graph_cycle_detect_true() {
        let mut g = super::Xg46Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_46 heap tests -------------------------------------------------

    #[test]
    fn xg_46_heap_empty() {
        let h: super::Xg46Heap<i32> = super::Xg46Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_46_heap_push_pop() {
        let mut h = super::Xg46Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_46_heap_peek() {
        let mut h = super::Xg46Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_46_heap_drain_sorted() {
        let mut h = super::Xg46Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_46_heap_merge() {
        let mut a = super::Xg46Heap::new();
        let mut b = super::Xg46Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_46_heap_default() {
        let h: super::Xg46Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_46_graph_default() {
        let g: super::Xg46Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh24_skip_insert_contains() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh24_skip_remove() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh24_skip_len() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh24_skip_range_query() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh24_skip_floor_ceiling() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh24_skip_rank() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh24_skip_empty() {
        let sl = super::Xh24SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh24_skip_duplicates() {
        let mut sl = super::Xh24SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh24_bitset_set_test() {
        let mut bs = super::Xh24BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh24_bitset_clear_count() {
        let mut bs = super::Xh24BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh24_bitset_and_or_xor() {
        let mut a = super::Xh24BitSet::xh_new(128);
        let mut b = super::Xh24BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh24_bitset_iter_ones() {
        let mut bs = super::Xh24BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh24_bitset_first_last() {
        let mut bs = super::Xh24BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh24_bitset_empty() {
        let bs = super::Xh24BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi24_deque_push_pop_back() {
        let mut dq = super::Xi24Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi24_deque_push_pop_front() {
        let mut dq = super::Xi24Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi24_deque_mixed_ops() {
        let mut dq = super::Xi24Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi24_deque_get_and_split() {
        let mut dq = super::Xi24Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi24_deque_rotate_left() {
        let mut dq = super::Xi24Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi24_deque_rotate_right() {
        let mut dq = super::Xi24Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi24_deque_grow() {
        let mut dq = super::Xi24Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi24_deque_empty() {
        let dq = super::Xi24Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi24_interval_tree_insert_query() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi24Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi24Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi24_interval_tree_overlap() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi24Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi24Interval::xi_new(12, 20));
        let q = super::Xi24Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi24_interval_tree_remove() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi24Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi24_interval_tree_gaps() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi24Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi24Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi24Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi24Interval::xi_new(8, 10));
    }

    #[test]
    fn xi24_interval_tree_merge() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi24Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi24Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi24Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi24Interval::xi_new(10, 15));
    }

    #[test]
    fn xi24_interval_tree_all() {
        let mut tree = super::Xi24IntervalTree::xi_new();
        tree.xi_insert(super::Xi24Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi24Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi24_interval_tree_empty() {
        let tree = super::Xi24IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi24_interval_tree_contains_point() {
        let iv = super::Xi24Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 25) ---

    #[test]
    fn xj_25_uf_make_and_find() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_25_uf_union_connected() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_25_uf_component_count() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_25_uf_component_size() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_25_uf_largest_component() {
        let mut uf = super::Xj25UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_25_uf_many_elements() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_25_uf_separate_components() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_25_uf_path_compression() {
        let mut uf = super::Xj25UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_25_bt_insert_get() {
        let mut bt = super::Xj25BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_25_bt_contains_len() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_25_bt_replace() {
        let mut bt = super::Xj25BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_25_bt_remove() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_25_bt_keys_values() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_25_bt_range() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_25_bt_min_max() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_25_bt_many_inserts() {
        let mut bt = super::Xj25BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_25 segment tree tests ---

    #[test]
    fn xk_25_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_25_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk25SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_25_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_25_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_25_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_25_st_single_element() {
        let data = vec![42];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_25_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk25SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_25_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk25SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_25 disjoint intervals tests ---

    #[test]
    fn xk_25_di_add_and_count() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_25_di_merge_overlap() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_25_di_contains() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_25_di_remove() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_25_di_covered_length() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_25_di_gaps() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_25_di_merge_adjacent() {
        let mut di = super::Xk25DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_25_di_empty() {
        let di = super::Xk25DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_25_rope_new_empty() {
        let rope = super::Xl25Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_25_rope_from_str() {
        let rope = super::Xl25Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_25_rope_insert_at() {
        let mut rope = super::Xl25Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_25_rope_delete_range() {
        let mut rope = super::Xl25Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_25_rope_char_at() {
        let rope = super::Xl25Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_25_rope_split_concat() {
        let rope = super::Xl25Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_25_rope_line_count() {
        let rope = super::Xl25Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_25_rope_line_at() {
        let rope = super::Xl25Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_25_sa_build_and_search() {
        let sa = super::Xl25SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_25_sa_count() {
        let sa = super::Xl25SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_25_sa_longest_repeated() {
        let sa = super::Xl25SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_25_sa_all_positions() {
        let sa = super::Xl25SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_25_sa_len() {
        let sa = super::Xl25SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_25_sa_empty() {
        let sa = super::Xl25SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_25_rope_slice() {
        let rope = super::Xl25Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_25_sa_search_start() {
        let sa = super::Xl25SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_25_sparse_set_get() {
        let mut m = super::Xm25MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_25_sparse_row_col() {
        let mut m = super::Xm25MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_25_sparse_transpose() {
        let mut m = super::Xm25MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_25_sparse_multiply_vec() {
        let mut m = super::Xm25MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_25_sparse_nnz_density() {
        let mut m = super::Xm25MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_25_sparse_clear() {
        let mut m = super::Xm25MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_25_sparse_overwrite_zero() {
        let mut m = super::Xm25MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_25_tokenizer_basic() {
        let t = super::Xm25Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_25_tokenizer_count() {
        let t = super::Xm25Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_25_tokenizer_unique() {
        let t = super::Xm25Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_25_tokenizer_frequency() {
        let t = super::Xm25Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_25_tokenizer_delimiter() {
        let t = super::Xm25Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_25_tokenizer_whitespace() {
        let t = super::Xm25Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_25_tokenizer_empty() {
        let t = super::Xm25Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 24 ----

    #[test]
    fn xn_24_fenwick_prefix_sum() {
        let mut ft = super::Xn24Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_24_fenwick_range_sum() {
        let mut ft = super::Xn24Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_24_fenwick_point_query() {
        let mut ft = super::Xn24Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_24_fenwick_len() {
        let ft = super::Xn24Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_24_fenwick_multiple_updates() {
        let mut ft = super::Xn24Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_24_fenwick_single_element() {
        let mut ft = super::Xn24Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_24_fenwick_find_kth() {
        let mut ft = super::Xn24Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_24_fenwick_negative_delta() {
        let mut ft = super::Xn24Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 24 ----

    #[test]
    fn xn_24_avl_insert_get() {
        let mut m = super::Xn24AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_24_avl_remove() {
        let mut m = super::Xn24AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_24_avl_in_order() {
        let mut m = super::Xn24AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_24_avl_min_max() {
        let mut m = super::Xn24AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_24_avl_floor_ceiling() {
        let mut m = super::Xn24AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_24_avl_height_balanced() {
        let mut m = super::Xn24AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_24_avl_overwrite() {
        let mut m = super::Xn24AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_24_avl_empty() {
        let m: super::Xn24AVL<i32, i32> = super::Xn24AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
