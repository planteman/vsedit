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

}
