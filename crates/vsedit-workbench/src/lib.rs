//! Main workbench shell — wires together layout, views, statusbar, editor
//! groups, and drives top-level rendering.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui::Frame;

use vsedit_quickinput::fuzzy_match;

use vsedit_debug_view::{DebugConsoleEntry, DebugView, OutputCategory};
use vsedit_output::OutputPanel;
use vsedit_problems::{Problem, ProblemSeverity, ProblemsPanel};
use vsedit_scm_view::{ScmGroup, ScmView};
use vsedit_search_view::SearchView;
use vsedit_terminal_view::TerminalView;

use vsedit_commands::{CommandRegistration, CommandRegistry};
use vsedit_contextkey::{ContextKeyValue, IContext};
use vsedit_editor_svc::EditorTabService;
use vsedit_explorer::ExplorerView;
use vsedit_input::{key_input_to_chord, InputEvent, KeyInput};
use vsedit_keybinding_svc::{
    register_default_keybindings, KeybindingResolver, KeybindingRule, KeybindingSource,
    KeybindingWeight, ResolveResult,
};
use vsedit_keycodes::{KeyCode, KeyCodeChord};
use vsedit_wb_layout::WorkbenchLayout;
use vsedit_wb_statusbar::{register_default_items, StatusBarService};
use vsedit_wb_textmate::{syntect_to_ratatui_color, TextMateService};
use vsedit_wb_views::{register_default_containers, ViewContainerLocation, ViewsRegistry};

// ---------------------------------------------------------------------------
// SplitDirection
// ---------------------------------------------------------------------------

/// Direction in which to split an editor group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Left,
    Right,
    Up,
    Down,
}

// ---------------------------------------------------------------------------
// EditorGroupOrientation
// ---------------------------------------------------------------------------

/// Orientation of an editor group within a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGroupOrientation {
    Horizontal,
    Vertical,
}

// ---------------------------------------------------------------------------
// EditorGroupTab
// ---------------------------------------------------------------------------

/// A single tab within an editor group.
#[derive(Debug, Clone)]
pub struct EditorGroupTab {
    pub title: String,
    pub file_path: Option<String>,
    pub content: String,
    pub is_modified: bool,
}

// ---------------------------------------------------------------------------
// EditorGroup
// ---------------------------------------------------------------------------

/// An editor group containing tabs, analogous to VS Code split editors.
#[derive(Debug, Clone)]
pub struct EditorGroup {
    pub group_id: usize,
    pub tabs: Vec<EditorGroupTab>,
    pub active_tab_idx: usize,
    pub orientation: EditorGroupOrientation,
}

impl EditorGroup {
    /// Create a new empty editor group.
    pub fn new(group_id: usize) -> Self {
        Self {
            group_id,
            tabs: Vec::new(),
            active_tab_idx: 0,
            orientation: EditorGroupOrientation::Horizontal,
        }
    }

    /// Add a tab to this group.
    pub fn add_tab(&mut self, tab: EditorGroupTab) {
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
    }

    /// Close a tab by index, returning it.
    pub fn close_tab(&mut self, idx: usize) -> Option<EditorGroupTab> {
        if idx >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(idx);
        if self.active_tab_idx >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab_idx = self.tabs.len() - 1;
        }
        Some(tab)
    }

    /// Get the active tab, if any.
    pub fn active_tab(&self) -> Option<&EditorGroupTab> {
        self.tabs.get(self.active_tab_idx)
    }

    /// Whether this group has no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// EditorGroupLayout
// ---------------------------------------------------------------------------

/// Layout arrangement of editor groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorGroupLayout {
    Single,
    Horizontal(Vec<usize>),
    Vertical(Vec<usize>),
    Grid(Vec<Vec<usize>>),
}

// ---------------------------------------------------------------------------
// EditorGroupManager
// ---------------------------------------------------------------------------

/// Manages multiple editor groups for split editor support.
pub struct EditorGroupManager {
    pub groups: Vec<EditorGroup>,
    pub active_group_id: usize,
    pub layout: EditorGroupLayout,
    next_id: usize,
}

impl EditorGroupManager {
    /// Create a manager with a single default group.
    pub fn new() -> Self {
        let group = EditorGroup::new(0);
        Self {
            groups: vec![group],
            active_group_id: 0,
            layout: EditorGroupLayout::Single,
            next_id: 1,
        }
    }

    /// Get a reference to the active editor group.
    pub fn active_group(&self) -> Option<&EditorGroup> {
        self.groups.iter().find(|g| g.group_id == self.active_group_id)
    }

    /// Get a mutable reference to the active editor group.
    pub fn active_group_mut(&mut self) -> Option<&mut EditorGroup> {
        self.groups.iter_mut().find(|g| g.group_id == self.active_group_id)
    }

    /// Get a group by ID.
    pub fn get_group(&self, group_id: usize) -> Option<&EditorGroup> {
        self.groups.iter().find(|g| g.group_id == group_id)
    }

    /// Split the current editor group in the given direction.
    pub fn split_editor(&mut self, direction: SplitDirection) -> usize {
        let new_id = self.next_id;
        self.next_id += 1;
        let new_group = EditorGroup::new(new_id);
        self.groups.push(new_group);

        // Transfer the active tab (clone) to the new group if one exists.
        if let Some(active) = self.groups.iter().find(|g| g.group_id == self.active_group_id) {
            if let Some(tab) = active.active_tab() {
                let cloned = EditorGroupTab {
                    title: tab.title.clone(),
                    file_path: tab.file_path.clone(),
                    content: tab.content.clone(),
                    is_modified: false,
                };
                if let Some(new_g) = self.groups.iter_mut().find(|g| g.group_id == new_id) {
                    new_g.add_tab(cloned);
                }
            }
        }

        // Update layout
        let ids: Vec<usize> = self.groups.iter().map(|g| g.group_id).collect();
        match direction {
            SplitDirection::Left | SplitDirection::Right => {
                self.layout = EditorGroupLayout::Horizontal(ids);
            }
            SplitDirection::Up | SplitDirection::Down => {
                self.layout = EditorGroupLayout::Vertical(ids);
            }
        }

        self.active_group_id = new_id;
        new_id
    }

    /// Close a group by ID and redistribute its tabs to the previous group.
    pub fn close_group(&mut self, group_id: usize) -> bool {
        if self.groups.len() <= 1 {
            return false;
        }
        let idx = match self.groups.iter().position(|g| g.group_id == group_id) {
            Some(i) => i,
            None => return false,
        };
        let removed = self.groups.remove(idx);
        // Find the target group to receive orphaned tabs.
        let target_id = if self.active_group_id == group_id {
            self.groups[0].group_id
        } else {
            self.active_group_id
        };
        if let Some(target) = self.groups.iter_mut().find(|g| g.group_id == target_id) {
            for tab in removed.tabs {
                target.add_tab(tab);
            }
        }
        if self.active_group_id == group_id {
            self.active_group_id = self.groups[0].group_id;
        }
        // Update layout
        if self.groups.len() == 1 {
            self.layout = EditorGroupLayout::Single;
        } else {
            let ids: Vec<usize> = self.groups.iter().map(|g| g.group_id).collect();
            match &self.layout {
                EditorGroupLayout::Vertical(_) => {
                    self.layout = EditorGroupLayout::Vertical(ids);
                }
                _ => {
                    self.layout = EditorGroupLayout::Horizontal(ids);
                }
            }
        }
        true
    }

    /// Move a tab from one group to another.
    pub fn move_editor_to_group(
        &mut self,
        from_group: usize,
        to_group: usize,
        tab_idx: usize,
    ) -> bool {
        if from_group == to_group {
            return false;
        }
        let tab = {
            let from = match self.groups.iter_mut().find(|g| g.group_id == from_group) {
                Some(g) => g,
                None => return false,
            };
            match from.close_tab(tab_idx) {
                Some(t) => t,
                None => return false,
            }
        };
        match self.groups.iter_mut().find(|g| g.group_id == to_group) {
            Some(to) => {
                to.add_tab(tab);
                true
            }
            None => false,
        }
    }

    /// Focus a specific group by ID.
    pub fn focus_group(&mut self, group_id: usize) -> bool {
        if self.groups.iter().any(|g| g.group_id == group_id) {
            self.active_group_id = group_id;
            true
        } else {
            false
        }
    }

    /// Get the number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

impl Default for EditorGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActivityBarItem
// ---------------------------------------------------------------------------

/// A single item in the activity bar with badge support.
#[derive(Debug, Clone)]
pub struct ActivityBarItem {
    pub id: String,
    pub label: String,
    pub icon_char: &'static str,
    pub badge: Option<usize>,
    pub is_active: bool,
}

impl ActivityBarItem {
    /// Create a new activity bar item.
    pub fn new(id: &str, label: &str, icon_char: &'static str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            icon_char,
            badge: None,
            is_active: false,
        }
    }

    /// Set the badge count.
    pub fn set_badge(&mut self, count: usize) {
        self.badge = if count > 0 { Some(count) } else { None };
    }

    /// Render the display text for this item.
    pub fn display_text(&self) -> String {
        match self.badge {
            Some(n) => format!("{}{}", self.icon_char, n),
            None => self.icon_char.to_string(),
        }
    }
}

/// Create the default set of activity bar items.
pub fn default_activity_bar_items() -> Vec<ActivityBarItem> {
    vec![
        ActivityBarItem::new("workbench.view.explorer", "Explorer", "📁"),
        ActivityBarItem::new("workbench.view.search", "Search", "🔍"),
        ActivityBarItem::new("workbench.view.scm", "Source Control", "🔀"),
        ActivityBarItem::new("workbench.view.debug", "Run and Debug", "🐛"),
        ActivityBarItem::new("workbench.view.extensions", "Extensions", "📦"),
    ]
}

// ---------------------------------------------------------------------------
// BreadcrumbItem
// ---------------------------------------------------------------------------

/// Kind of breadcrumb item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreadcrumbKind {
    File,
    Folder,
    Symbol,
}

/// A single breadcrumb in the path trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub path: String,
}

impl BreadcrumbItem {
    /// Create a new breadcrumb item.
    pub fn new(label: &str, kind: BreadcrumbKind, path: &str) -> Self {
        Self {
            label: label.to_string(),
            kind,
            path: path.to_string(),
        }
    }
}

/// Compute breadcrumbs from a file path and optional symbols.
pub fn compute_breadcrumbs(file_path: &str, symbols: &[String]) -> Vec<BreadcrumbItem> {
    let path = std::path::Path::new(file_path);
    let mut crumbs = Vec::new();

    // Add folder components
    if let Some(parent) = path.parent() {
        let mut accumulated = String::new();
        for component in parent.components() {
            let name = component.as_os_str().to_string_lossy().to_string();
            if name == "/" || name == "." {
                continue;
            }
            if !accumulated.is_empty() {
                accumulated.push('/');
            }
            accumulated.push_str(&name);
            crumbs.push(BreadcrumbItem::new(&name, BreadcrumbKind::Folder, &accumulated));
        }
    }

    // Add the file itself
    if let Some(filename) = path.file_name() {
        crumbs.push(BreadcrumbItem::new(
            &filename.to_string_lossy(),
            BreadcrumbKind::File,
            file_path,
        ));
    }

    // Add symbol breadcrumbs
    for sym in symbols {
        crumbs.push(BreadcrumbItem::new(sym, BreadcrumbKind::Symbol, file_path));
    }

    crumbs
}

/// Render breadcrumbs into the given buffer area.
pub fn render_breadcrumbs(crumbs: &[BreadcrumbItem], area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 || crumbs.is_empty() {
        return;
    }
    let mut spans: Vec<Span> = Vec::new();
    for (i, crumb) in crumbs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" › ", Style::default().fg(Color::DarkGray)));
        }
        let style = match crumb.kind {
            BreadcrumbKind::Folder => Style::default().fg(Color::DarkGray),
            BreadcrumbKind::File => Style::default().fg(Color::White),
            BreadcrumbKind::Symbol => Style::default().fg(Color::Yellow),
        };
        spans.push(Span::styled(&crumb.label, style));
    }
    let line = Line::from(spans);
    let para = Paragraph::new(line);
    para.render(area, buf);
}

// ---------------------------------------------------------------------------
// Minimap
// ---------------------------------------------------------------------------

/// Render a text minimap using Braille characters.
///
/// Each minimap row represents `LINES_PER_ROW` editor lines. The current
/// viewport is highlighted with a different background.
pub fn render_minimap(
    lines: &[String],
    viewport_start: usize,
    viewport_height: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 || lines.is_empty() {
        return;
    }

    const LINES_PER_ROW: usize = 4;
    let minimap_cols = area.width as usize;

    for row in 0..area.height as usize {
        let start_line = row * LINES_PER_ROW;
        if start_line >= lines.len() {
            break;
        }

        // Determine if this minimap row overlaps the viewport
        let end_line = (start_line + LINES_PER_ROW).min(lines.len());
        let in_viewport = start_line < viewport_start + viewport_height
            && end_line > viewport_start;

        let bg = if in_viewport { Color::DarkGray } else { Color::Black };

        // Build a condensed representation using block characters
        let mut text = String::new();
        for col in 0..minimap_cols {
            let mut has_content = false;
            for ln in start_line..end_line {
                if let Some(line) = lines.get(ln) {
                    if col < line.len() && !line.as_bytes().get(col).map_or(true, |b| *b == b' ') {
                        has_content = true;
                        break;
                    }
                }
            }
            text.push(if has_content { '▒' } else { ' ' });
        }

        let y = area.y + row as u16;
        if y >= area.y + area.height {
            break;
        }
        let style = Style::default().fg(Color::Gray).bg(bg);
        buf.set_string(area.x, y, &text, style);
    }
}

// ---------------------------------------------------------------------------
// TitleBar
// ---------------------------------------------------------------------------

/// Compute the title bar text for the workbench.
pub fn compute_title_bar(
    file_path: Option<&str>,
    folder: Option<&str>,
    is_modified: bool,
) -> String {
    let dirty = if is_modified { "● " } else { "" };
    match (file_path, folder) {
        (Some(fp), Some(dir)) => {
            let filename = std::path::Path::new(fp)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| fp.to_string());
            format!("{}{} — {} — vsedit", dirty, filename, dir)
        }
        (Some(fp), None) => {
            let filename = std::path::Path::new(fp)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| fp.to_string());
            format!("{}{} — vsedit", dirty, filename)
        }
        (None, Some(dir)) => format!("{} — vsedit", dir),
        (None, None) => "vsedit".to_string(),
    }
}

// ---------------------------------------------------------------------------
// ActivePanelView
// ---------------------------------------------------------------------------

/// Which sub-view is active inside the bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanelView {
    Terminal,
    Problems,
    Output,
    DebugConsole,
}

impl ActivePanelView {
    fn label(&self) -> &'static str {
        match self {
            Self::Terminal => "TERMINAL",
            Self::Problems => "PROBLEMS",
            Self::Output => "OUTPUT",
            Self::DebugConsole => "DEBUG CONSOLE",
        }
    }

    fn all() -> &'static [ActivePanelView] {
        &[
            ActivePanelView::Terminal,
            ActivePanelView::Problems,
            ActivePanelView::Output,
            ActivePanelView::DebugConsole,
        ]
    }

    fn next(&self) -> Self {
        match self {
            Self::Terminal => Self::Problems,
            Self::Problems => Self::Output,
            Self::Output => Self::DebugConsole,
            Self::DebugConsole => Self::Terminal,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Self::Terminal => Self::DebugConsole,
            Self::Problems => Self::Terminal,
            Self::Output => Self::Problems,
            Self::DebugConsole => Self::Output,
        }
    }
}

// ---------------------------------------------------------------------------
// ActiveSidebarPanel
// ---------------------------------------------------------------------------

/// Which view is active inside the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSidebarPanel {
    Explorer,
    Search,
    SourceControl,
    Debug,
    Extensions,
}

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
// CommandPaletteItem
// ---------------------------------------------------------------------------

/// A single entry in the command palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
    pub id: String,
    pub label: String,
    pub keybinding: Option<String>,
    pub score: f64,
}

// ---------------------------------------------------------------------------
// QuickOpenState
// ---------------------------------------------------------------------------

/// State for the Quick Open file picker overlay.
#[derive(Debug, Clone)]
pub struct QuickOpenState {
    /// Current query typed by the user.
    pub query: String,
    /// Filtered file paths matching the query.
    pub filtered_results: Vec<PathBuf>,
    /// Currently selected index in the results list.
    pub selected_index: usize,
}

impl QuickOpenState {
    /// Create a new empty Quick Open state.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            filtered_results: Vec::new(),
            selected_index: 0,
        }
    }

    /// Filter the given workspace files using fuzzy matching against the query.
    pub fn filter(&mut self, workspace_files: &[PathBuf]) {
        if self.query.is_empty() {
            self.filtered_results = workspace_files.to_vec();
        } else {
            let mut scored: Vec<(PathBuf, i32)> = workspace_files
                .iter()
                .filter_map(|p| {
                    let name = p.to_string_lossy();
                    fuzzy_match(&self.query, &name).map(|m| (p.clone(), m.score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered_results = scored.into_iter().map(|(p, _)| p).collect();
        }
        self.selected_index = 0;
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if !self.filtered_results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.filtered_results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_results.len();
        }
    }

    /// Get the currently selected file path, if any.
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.filtered_results.get(self.selected_index)
    }
}

impl Default for QuickOpenState {
    fn default() -> Self {
        Self::new()
    }
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
    /// TextMate syntax highlighting service.
    textmate: TextMateService,
    /// Detected syntax name for the current file.
    syntax_name: Option<String>,
    /// Whether the command palette overlay is visible.
    pub show_command_palette: bool,
    /// Current text typed into the command palette input.
    pub command_palette_input: String,
    /// Items shown in the command palette (filtered).
    pub command_palette_items: Vec<CommandPaletteItem>,
    /// Currently selected index in the command palette.
    pub command_palette_selected: usize,
    /// Saved focus to restore when closing the palette.
    saved_focus: FocusedPart,
    /// Tab management service.
    pub tab_service: EditorTabService,
    /// Optional file explorer for the sidebar.
    pub explorer: Option<ExplorerView>,
    /// Which sidebar panel is active.
    pub active_sidebar: ActiveSidebarPanel,
    /// Search view for the sidebar.
    pub search_view: SearchView,
    /// Source control view for the sidebar.
    pub scm_view: ScmView,
    /// SCM groups cache for rendering.
    pub scm_groups: Vec<ScmGroup>,
    /// SCM branch name cache.
    pub scm_branch: Option<String>,
    /// Debug view for the sidebar.
    pub debug_view: DebugView,
    /// Which sub-view is active in the bottom panel.
    pub active_panel: ActivePanelView,
    /// Terminal panel view.
    pub terminal_view: TerminalView,
    /// Problems panel view.
    pub problems_panel: ProblemsPanel,
    /// Output panel view.
    pub output_panel: OutputPanel,
    /// Editor group manager for split editors.
    pub editor_groups: EditorGroupManager,
    /// Activity bar items with badge support.
    pub activity_bar_items: Vec<ActivityBarItem>,
    /// Computed breadcrumb trail for the current file.
    pub breadcrumbs: Vec<BreadcrumbItem>,
    /// Current viewport scroll offset (line number of first visible line).
    pub viewport_scroll: usize,
    /// Working directory for title bar.
    pub workspace_folder: Option<String>,
    /// Quick Open file picker state.
    pub quick_open: QuickOpenState,
    /// Whether the Quick Open overlay is visible.
    pub show_quick_open: bool,
    /// Known workspace file paths for Quick Open filtering.
    pub workspace_files: Vec<PathBuf>,
    /// Whether the Go To Line input overlay is visible.
    pub show_goto_line: bool,
    /// Current text typed into the Go To Line input.
    pub goto_line_input: String,
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
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyJ,
            )),
            command: "workbench.action.togglePanel".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyQ,
            )),
            command: "workbench.action.quit".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Tab navigation keybindings
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Tab,
            )),
            command: "workbench.action.nextEditor".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::Tab,
            )),
            command: "workbench.action.previousEditor".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyW,
            )),
            command: "workbench.action.closeActiveEditor".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Panel keybindings
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Backquote,
            )),
            command: "workbench.action.terminal.toggleTerminal".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyM,
            )),
            command: "workbench.action.problems.focus".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyU,
            )),
            command: "workbench.action.output.focus".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Sidebar panel keybindings
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyE,
            )),
            command: "workbench.view.explorer".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyF,
            )),
            command: "workbench.view.search".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyG,
            )),
            command: "workbench.view.scm".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyD,
            )),
            command: "workbench.view.debug".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::KeyX,
            )),
            command: "workbench.view.extensions".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Editor split keybinding: Ctrl+\
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Backslash,
            )),
            command: "workbench.action.splitEditor".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Focus group keybindings: Ctrl+1/2/3
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Digit1,
            )),
            command: "workbench.action.focusFirstEditorGroup".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Digit2,
            )),
            command: "workbench.action.focusSecondEditorGroup".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::Digit3,
            )),
            command: "workbench.action.focusThirdEditorGroup".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        // Resize sidebar: Ctrl+Shift+Left/Right
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::LeftArrow,
            )),
            command: "workbench.action.decreaseSidebarWidth".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, true, false, false, KeyCode::RightArrow,
            )),
            command: "workbench.action.increaseSidebarWidth".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
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
            commands.register("workbench.action.showCommands", Box::new(|_| Ok(None))),
            commands.register("workbench.action.quickOpen", Box::new(|_| Ok(None))),
            commands.register("workbench.action.files.newUntitledFile", Box::new(|_| Ok(None))),
            commands.register("workbench.action.files.openFile", Box::new(|_| Ok(None))),
            commands.register("workbench.action.files.save", Box::new(|_| Ok(None))),
            commands.register("workbench.action.nextEditor", Box::new(|_| Ok(None))),
            commands.register("workbench.action.previousEditor", Box::new(|_| Ok(None))),
            commands.register("workbench.action.closeActiveEditor", Box::new(|_| Ok(None))),
            commands.register("workbench.action.terminal.toggleTerminal", Box::new(|_| Ok(None))),
            commands.register("workbench.action.problems.focus", Box::new(|_| Ok(None))),
            commands.register("workbench.action.output.focus", Box::new(|_| Ok(None))),
            commands.register("workbench.action.panel.nextTab", Box::new(|_| Ok(None))),
            commands.register("workbench.action.panel.previousTab", Box::new(|_| Ok(None))),
            commands.register("workbench.view.explorer", Box::new(|_| Ok(None))),
            commands.register("workbench.view.search", Box::new(|_| Ok(None))),
            commands.register("workbench.view.scm", Box::new(|_| Ok(None))),
            commands.register("workbench.view.debug", Box::new(|_| Ok(None))),
            commands.register("workbench.view.extensions", Box::new(|_| Ok(None))),
            commands.register("workbench.action.splitEditor", Box::new(|_| Ok(None))),
            commands.register("workbench.action.focusFirstEditorGroup", Box::new(|_| Ok(None))),
            commands.register("workbench.action.focusSecondEditorGroup", Box::new(|_| Ok(None))),
            commands.register("workbench.action.focusThirdEditorGroup", Box::new(|_| Ok(None))),
            commands.register("workbench.action.decreaseSidebarWidth", Box::new(|_| Ok(None))),
            commands.register("workbench.action.increaseSidebarWidth", Box::new(|_| Ok(None))),
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
            textmate: TextMateService::new(),
            syntax_name: None,
            show_command_palette: false,
            command_palette_input: String::new(),
            command_palette_items: Vec::new(),
            command_palette_selected: 0,
            saved_focus: FocusedPart::Editor,
            tab_service: EditorTabService::new(),
            explorer: None,
            active_sidebar: ActiveSidebarPanel::Explorer,
            search_view: SearchView::new(),
            scm_view: ScmView::new(),
            scm_groups: Vec::new(),
            scm_branch: None,
            debug_view: DebugView::new(),
            active_panel: ActivePanelView::Terminal,
            terminal_view: TerminalView::new(),
            problems_panel: ProblemsPanel::new(),
            output_panel: OutputPanel::new(),
            editor_groups: EditorGroupManager::new(),
            activity_bar_items: default_activity_bar_items(),
            breadcrumbs: Vec::new(),
            viewport_scroll: 0,
            workspace_folder: None,
            quick_open: QuickOpenState::new(),
            show_quick_open: false,
            workspace_files: Vec::new(),
            show_goto_line: false,
            goto_line_input: String::new(),
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
        self.file_path = path.clone();
        // Compute breadcrumbs from file path
        if let Some(ref p) = path {
            self.breadcrumbs = compute_breadcrumbs(p, &[]);
        } else {
            self.breadcrumbs.clear();
        }
        // Detect syntax from file path
        if let Some(ref p) = path {
            let file_path = std::path::Path::new(p);
            if let Some(syntax) = self.textmate.find_syntax_for_file(file_path) {
                let name = syntax.name.clone();
                self.statusbar.update_item("statusbar.language", &name);
                self.syntax_name = Some(name);
            } else {
                self.statusbar.update_item("statusbar.language", "Plain Text");
                self.syntax_name = None;
            }
        } else {
            self.statusbar.update_item("statusbar.language", "Plain Text");
            self.syntax_name = None;
        }
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
            let title_text = compute_title_bar(
                self.file_path.as_deref(),
                self.workspace_folder.as_deref(),
                self.is_modified,
            );
            let title = Paragraph::new(Line::from(vec![
                Span::styled(title_text, Style::default().fg(Color::Cyan)),
            ]))
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::DarkGray));
            frame.render_widget(title, menubar);
        }

        // Activity bar (using activity_bar_items with badge support)
        if let Some(ab) = result.activity_bar {
            let icons: Vec<Line> = self.activity_bar_items
                .iter()
                .map(|item| {
                    let is_active = match (self.active_sidebar, item.id.as_str()) {
                        (ActiveSidebarPanel::Explorer, "workbench.view.explorer") => true,
                        (ActiveSidebarPanel::Search, "workbench.view.search") => true,
                        (ActiveSidebarPanel::SourceControl, "workbench.view.scm") => true,
                        (ActiveSidebarPanel::Debug, "workbench.view.debug") => true,
                        (ActiveSidebarPanel::Extensions, "workbench.view.extensions") => true,
                        _ => false,
                    };
                    let text = item.display_text();
                    let style = if is_active {
                        Style::default().fg(Color::White).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Gray).bg(Color::DarkGray)
                    };
                    Line::from(Span::styled(text, style))
                })
                .collect();
            let activity = Paragraph::new(icons).style(Style::default().bg(Color::DarkGray));
            frame.render_widget(activity, ab);
        }

        // Sidebar
        if let Some(sb) = result.sidebar {
            let active_title = match self.active_sidebar {
                ActiveSidebarPanel::Explorer => "EXPLORER".to_string(),
                ActiveSidebarPanel::Search => "SEARCH".to_string(),
                ActiveSidebarPanel::SourceControl => "SOURCE CONTROL".to_string(),
                ActiveSidebarPanel::Debug => "RUN AND DEBUG".to_string(),
                ActiveSidebarPanel::Extensions => "EXTENSIONS".to_string(),
            };

            let block = Block::default().borders(Borders::RIGHT);
            let inner = block.inner(sb);
            frame.render_widget(
                block.style(Style::default().bg(Color::Black)),
                sb,
            );

            if inner.height > 0 && inner.width > 0 {
                let title_area = Rect::new(inner.x, inner.y, inner.width, 1);
                let title_para = Paragraph::new(Span::styled(
                    active_title,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ));
                frame.render_widget(title_para, title_area);

                if inner.height > 1 {
                    let content_area = Rect::new(
                        inner.x,
                        inner.y + 1,
                        inner.width,
                        inner.height - 1,
                    );
                    match self.active_sidebar {
                        ActiveSidebarPanel::Explorer => {
                            if let Some(ref explorer) = self.explorer {
                                explorer.render(content_area, frame.buffer_mut());
                            }
                        }
                        ActiveSidebarPanel::Search => {
                            self.search_view.render(content_area, frame.buffer_mut());
                        }
                        ActiveSidebarPanel::SourceControl => {
                            self.scm_view.render(
                                &self.scm_groups,
                                self.scm_branch.as_deref(),
                                content_area,
                                frame.buffer_mut(),
                            );
                        }
                        ActiveSidebarPanel::Debug => {
                            self.debug_view.render(content_area, frame.buffer_mut());
                        }
                        ActiveSidebarPanel::Extensions => {
                            let stub = Paragraph::new(Span::styled(
                                " No extensions installed",
                                Style::default().fg(Color::DarkGray),
                            ));
                            frame.render_widget(stub, content_area);
                        }
                    }
                }
            }
        }

        // Editor area (with tab bar, breadcrumbs, and minimap)
        {
            let editor_rect = result.editor;
            let mut current_y = editor_rect.y;
            let mut remaining_h = editor_rect.height;

            // Tab bar
            if self.tab_service.tab_count() > 0 && remaining_h > 1 {
                let tab_area = Rect::new(editor_rect.x, current_y, editor_rect.width, 1);
                let mut tab_spans: Vec<Span> = Vec::new();
                for tab in self.tab_service.get_tabs() {
                    let indicator = if tab.is_modified { " ●" } else { " ✕" };
                    let label = format!(" {}{} ", tab.title, indicator);
                    let style = if tab.is_active {
                        Style::default().fg(Color::White).bg(Color::Black)
                    } else {
                        Style::default().fg(Color::DarkGray).bg(Color::Black)
                    };
                    tab_spans.push(Span::styled(label, style));
                }
                let tab_line = Paragraph::new(Line::from(tab_spans))
                    .style(Style::default().bg(Color::DarkGray));
                frame.render_widget(tab_line, tab_area);
                current_y += 1;
                remaining_h -= 1;
            }

            // Breadcrumb bar
            if !self.breadcrumbs.is_empty() && remaining_h > 1 {
                let bc_area = Rect::new(editor_rect.x, current_y, editor_rect.width, 1);
                render_breadcrumbs(&self.breadcrumbs, bc_area, frame.buffer_mut());
                current_y += 1;
                remaining_h -= 1;
            }

            // Split remaining into editor text + minimap
            let minimap_width: u16 = if remaining_h > 2 && editor_rect.width > 30 { 10 } else { 0 };
            let text_width = editor_rect.width.saturating_sub(minimap_width);
            let content_area = Rect::new(editor_rect.x, current_y, text_width, remaining_h);

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
                        let syntax_ref = self.syntax_name.as_deref().and_then(|name| {
                            self.textmate.find_syntax_by_name(name)
                        });
                        let mut highlighter = syntax_ref.map(|s| self.textmate.create_highlighter(s));

                        lines
                            .iter()
                            .enumerate()
                            .map(|(i, line)| {
                                let mut spans = vec![
                                    Span::styled(
                                        format!(" {:>width$} ", i + 1, width = num_width),
                                        Style::default().fg(Color::DarkGray),
                                    ),
                                ];
                                let line_with_nl = format!("{}\n", line);
                                if let Some(ref mut hl) = highlighter {
                                    let segments = self.textmate.highlight_line(hl, &line_with_nl);
                                    for (style, text) in segments {
                                        let trimmed = text.trim_end_matches('\n').to_string();
                                        if !trimmed.is_empty() {
                                            spans.push(Span::styled(
                                                trimmed,
                                                Style::default().fg(syntect_to_ratatui_color(style)),
                                            ));
                                        }
                                    }
                                } else {
                                    spans.push(Span::raw(line.as_str()));
                                }
                                Line::from(spans)
                            })
                            .collect()
                    };
                    let editor = Paragraph::new(rendered);
                    frame.render_widget(editor, content_area);

                    // Render minimap
                    if minimap_width > 0 && !lines.is_empty() {
                        let minimap_area = Rect::new(
                            editor_rect.x + text_width,
                            current_y,
                            minimap_width,
                            remaining_h,
                        );
                        render_minimap(
                            lines,
                            self.viewport_scroll,
                            remaining_h as usize,
                            minimap_area,
                            frame.buffer_mut(),
                        );
                    }
                }
                _ => {
                    let editor = Paragraph::new("No editors open")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(editor, content_area);
                }
            }
        }

        // Panel
        if let Some(panel) = result.panel {
            self.render_panel(frame, panel);
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

        // Command palette overlay
        if self.show_command_palette {
            self.render_command_palette(frame, area);
        }

        // Quick Open overlay
        if self.show_quick_open {
            self.render_quick_open(frame, area);
        }

        // Go To Line overlay
        if self.show_goto_line {
            self.render_goto_line(frame, area);
        }
    }

    /// Render the command palette overlay.
    fn render_command_palette(&self, frame: &mut Frame, area: Rect) {
        let width = (area.width * 3 / 5).max(20).min(area.width);
        let height = (area.height * 2 / 5).max(5).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + 1;
        let palette_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, palette_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::DarkGray));
        let inner = block.inner(palette_area);
        frame.render_widget(block, palette_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Input line
        let input_line = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(&self.command_palette_input),
        ]);
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(input_line), input_area);

        // Item list
        let list_start_y = inner.y + 1;
        let list_height = inner.height.saturating_sub(1) as usize;

        for (i, item) in self.command_palette_items.iter().take(list_height).enumerate() {
            let style = if i == self.command_palette_selected {
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let mut spans = vec![Span::styled(&item.label, style)];
            if let Some(ref kb) = item.keybinding {
                spans.push(Span::styled(
                    format!("  {kb}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let row_area = Rect::new(inner.x, list_start_y + i as u16, inner.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
        }
    }

    /// Render the bottom panel with tab bar and active sub-view.
    fn render_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::TOP);
        let inner = block.inner(area);
        frame.render_widget(block.style(Style::default().bg(Color::Black)), area);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        // Tab bar row
        let tab_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let mut tab_spans: Vec<Span> = Vec::new();
        for view in ActivePanelView::all() {
            let style = if *view == self.active_panel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            tab_spans.push(Span::styled(format!(" {} ", view.label()), style));
        }
        frame.render_widget(Paragraph::new(Line::from(tab_spans)), tab_area);

        // Content area below tab bar
        let content_area = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(1),
        );

        match self.active_panel {
            ActivePanelView::Terminal => {
                self.terminal_view.render(content_area, frame.buffer_mut());
            }
            ActivePanelView::Problems => {
                self.problems_panel.render(content_area, frame.buffer_mut());
            }
            ActivePanelView::Output => {
                self.output_panel.render(content_area, frame.buffer_mut());
            }
            ActivePanelView::DebugConsole => {
                let entries = self.debug_view.console.entries();
                if entries.is_empty() {
                    let empty = Paragraph::new(Span::styled(
                        "Debug Console (not connected)",
                        Style::default().fg(Color::DarkGray),
                    ));
                    frame.render_widget(empty, content_area);
                } else {
                    let max_lines = content_area.height.saturating_sub(1) as usize;
                    let skip = entries.len().saturating_sub(max_lines);
                    let lines: Vec<Line> = entries.iter().skip(skip).map(|entry| {
                        match entry {
                            DebugConsoleEntry::Input(text) => Line::from(vec![
                                Span::styled("> ", Style::default().fg(Color::Cyan)),
                                Span::styled(text.as_str(), Style::default().fg(Color::White)),
                            ]),
                            DebugConsoleEntry::Output(text, OutputCategory::Stderr) => {
                                Line::from(Span::styled(text.as_str(), Style::default().fg(Color::Red)))
                            }
                            DebugConsoleEntry::Output(text, OutputCategory::Stdout) => {
                                Line::from(Span::styled(text.as_str(), Style::default().fg(Color::White)))
                            }
                            DebugConsoleEntry::Output(text, _) => {
                                Line::from(Span::styled(text.as_str(), Style::default().fg(Color::Gray)))
                            }
                        }
                    }).collect();
                    let output_h = lines.len() as u16;
                    let output_area = Rect::new(
                        content_area.x,
                        content_area.y,
                        content_area.width,
                        output_h.min(content_area.height),
                    );
                    frame.render_widget(Paragraph::new(lines), output_area);
                    // Prompt line at bottom
                    let prompt_y = content_area.y + output_h.min(content_area.height.saturating_sub(1));
                    if prompt_y < content_area.y + content_area.height {
                        let prompt_area = Rect::new(
                            content_area.x, prompt_y, content_area.width, 1,
                        );
                        let prompt = Paragraph::new(Line::from(
                            Span::styled("> ", Style::default().fg(Color::Cyan)),
                        ));
                        frame.render_widget(prompt, prompt_area);
                    }
                }
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
        // When command palette is focused, handle keys directly.
        if self.focused == FocusedPart::CommandPalette {
            return self.handle_palette_key(key);
        }

        // When quick open or go-to-line is focused, handle keys directly.
        if self.focused == FocusedPart::QuickInput {
            if self.show_goto_line {
                if let Some(line) = self.handle_goto_line_key(key) {
                    return WorkbenchAction::ExecuteCommand(
                        format!("__gotoLine:{}", line),
                    );
                }
                return WorkbenchAction::None;
            }
            return self.handle_quick_open_key(key);
        }

        // When panel is focused, Left/Right switch panel tabs.
        if self.focused == FocusedPart::Panel && !key.ctrl && !key.alt && !key.meta {
            match key.key_code {
                KeyCode::RightArrow => {
                    self.active_panel = self.active_panel.next();
                    return WorkbenchAction::None;
                }
                KeyCode::LeftArrow => {
                    self.active_panel = self.active_panel.prev();
                    return WorkbenchAction::None;
                }
                _ => {}
            }
        }

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
                // Route unbound keys to sidebar when focused
                if self.focused == FocusedPart::Sidebar {
                    self.handle_sidebar_key(&key);
                }
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
            "workbench.action.showCommands" => {
                self.open_command_palette();
            }
            "workbench.action.quickOpen" => {
                self.open_quick_open();
            }
            "workbench.action.files.newUntitledFile" => {
                self.editor_content = Some(Vec::new());
                self.file_path = None;
                self.is_modified = false;
            }
            "workbench.action.files.openFile" => {
                self.statusbar.update_item("statusbar.notification", "Use: vsedit <file>");
            }
            "workbench.action.files.save" => {
                // Handled by caller
            }
            "workbench.action.nextEditor" => {
                self.tab_service.next_tab();
                self.sync_active_tab_to_editor();
            }
            "workbench.action.previousEditor" => {
                self.tab_service.previous_tab();
                self.sync_active_tab_to_editor();
            }
            "workbench.action.closeActiveEditor" => {
                if let Some(tab) = self.tab_service.get_active_tab() {
                    let id = tab.id;
                    self.tab_service.close_tab(id);
                    self.sync_active_tab_to_editor();
                }
            }
            "workbench.view.explorer" => {
                self.set_active_sidebar(ActiveSidebarPanel::Explorer);
            }
            "workbench.view.search" => {
                self.set_active_sidebar(ActiveSidebarPanel::Search);
            }
            "workbench.view.scm" => {
                self.set_active_sidebar(ActiveSidebarPanel::SourceControl);
            }
            "workbench.view.debug" => {
                self.set_active_sidebar(ActiveSidebarPanel::Debug);
            }
            "workbench.view.extensions" => {
                self.set_active_sidebar(ActiveSidebarPanel::Extensions);
            }
            "workbench.action.terminal.toggleTerminal" => {
                if self.focused == FocusedPart::Panel
                    && self.active_panel == ActivePanelView::Terminal
                {
                    self.focused = FocusedPart::Editor;
                } else {
                    self.active_panel = ActivePanelView::Terminal;
                    self.focused = FocusedPart::Panel;
                    if !self.layout.is_part_visible(vsedit_wb_layout::Part::Panel) {
                        self.layout.toggle_panel();
                    }
                }
            }
            "workbench.action.problems.focus" => {
                self.active_panel = ActivePanelView::Problems;
                self.focused = FocusedPart::Panel;
                if !self.layout.is_part_visible(vsedit_wb_layout::Part::Panel) {
                    self.layout.toggle_panel();
                }
            }
            "workbench.action.output.focus" => {
                self.active_panel = ActivePanelView::Output;
                self.focused = FocusedPart::Panel;
                if !self.layout.is_part_visible(vsedit_wb_layout::Part::Panel) {
                    self.layout.toggle_panel();
                }
            }
            "workbench.action.panel.nextTab" => {
                self.active_panel = self.active_panel.next();
            }
            "workbench.action.panel.previousTab" => {
                self.active_panel = self.active_panel.prev();
            }
            "workbench.action.splitEditor" => {
                self.editor_groups.split_editor(SplitDirection::Right);
            }
            "workbench.action.focusFirstEditorGroup" => {
                if let Some(g) = self.editor_groups.groups.first() {
                    let id = g.group_id;
                    self.editor_groups.focus_group(id);
                }
            }
            "workbench.action.focusSecondEditorGroup" => {
                if let Some(g) = self.editor_groups.groups.get(1) {
                    let id = g.group_id;
                    self.editor_groups.focus_group(id);
                }
            }
            "workbench.action.focusThirdEditorGroup" => {
                if let Some(g) = self.editor_groups.groups.get(2) {
                    let id = g.group_id;
                    self.editor_groups.focus_group(id);
                }
            }
            "workbench.action.decreaseSidebarWidth" => {
                let w = self.layout.get_sidebar_width();
                self.layout.set_sidebar_width(w.saturating_sub(2).max(10));
            }
            "workbench.action.increaseSidebarWidth" => {
                let w = self.layout.get_sidebar_width();
                self.layout.set_sidebar_width((w + 2).min(80));
            }
            _ => {
                let _ = self.commands.execute(command_id, vec![]);
            }
        }
    }

    /// Open the command palette and populate with available commands.
    pub fn open_command_palette(&mut self) {
        self.saved_focus = self.focused;
        self.focused = FocusedPart::CommandPalette;
        self.show_command_palette = true;
        self.command_palette_input.clear();
        self.command_palette_selected = 0;
        self.populate_palette_items();
    }

    /// Close the command palette and restore focus.
    pub fn close_command_palette(&mut self) {
        self.show_command_palette = false;
        self.command_palette_input.clear();
        self.command_palette_items.clear();
        self.command_palette_selected = 0;
        self.focused = self.saved_focus;
    }

    /// Populate palette items from the command registry, with keybinding info.
    fn populate_palette_items(&mut self) {
        let command_ids = self.commands.get_commands();
        self.command_palette_items = command_ids
            .into_iter()
            .map(|id| {
                let keybinding = self
                    .keybindings
                    .get_keybindings_for_command(&id)
                    .first()
                    .map(|rule| format!("{}", rule.keybinding));
                let label = command_id_to_label(&id);
                CommandPaletteItem {
                    id,
                    label,
                    keybinding,
                    score: 0.0,
                }
            })
            .collect();
    }

    /// Filter palette items by the current input using fuzzy matching.
    pub fn filter_palette_items(&mut self) {
        let input = self.command_palette_input.clone();
        let command_ids = self.commands.get_commands();
        let mut items: Vec<CommandPaletteItem> = Vec::new();

        for id in command_ids {
            let label = command_id_to_label(&id);
            let keybinding = self
                .keybindings
                .get_keybindings_for_command(&id)
                .first()
                .map(|rule| format!("{}", rule.keybinding));

            if input.is_empty() {
                items.push(CommandPaletteItem {
                    id,
                    label,
                    keybinding,
                    score: 0.0,
                });
            } else if let Some(m) = fuzzy_match(&input, &label) {
                items.push(CommandPaletteItem {
                    id,
                    label,
                    keybinding,
                    score: m.score as f64,
                });
            }
        }

        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        self.command_palette_items = items;
        self.command_palette_selected = 0;
    }

    /// Handle a key press while the command palette is focused.
    fn handle_palette_key(&mut self, key: KeyInput) -> WorkbenchAction {
        use vsedit_keycodes::KeyCode as KC;

        match key.key_code {
            KC::Escape => {
                self.close_command_palette();
                WorkbenchAction::None
            }
            KC::Enter => {
                if let Some(item) = self.command_palette_items.get(self.command_palette_selected) {
                    let cmd = item.id.clone();
                    self.close_command_palette();
                    WorkbenchAction::ExecuteCommand(cmd)
                } else {
                    self.close_command_palette();
                    WorkbenchAction::None
                }
            }
            KC::UpArrow => {
                if !self.command_palette_items.is_empty() {
                    if self.command_palette_selected == 0 {
                        self.command_palette_selected = self.command_palette_items.len() - 1;
                    } else {
                        self.command_palette_selected -= 1;
                    }
                }
                WorkbenchAction::None
            }
            KC::DownArrow => {
                if !self.command_palette_items.is_empty() {
                    self.command_palette_selected =
                        (self.command_palette_selected + 1) % self.command_palette_items.len();
                }
                WorkbenchAction::None
            }
            KC::Backspace => {
                self.command_palette_input.pop();
                self.filter_palette_items();
                WorkbenchAction::None
            }
            other => {
                if !key.ctrl && !key.alt && !key.meta {
                    if let Some(ch) = key_code_to_char(other, key.shift) {
                        self.command_palette_input.push(ch);
                        self.filter_palette_items();
                    }
                }
                WorkbenchAction::None
            }
        }
    }

    /// Open the Quick Open file picker overlay.
    pub fn open_quick_open(&mut self) {
        self.saved_focus = self.focused;
        self.focused = FocusedPart::QuickInput;
        self.show_quick_open = true;
        self.quick_open.query.clear();
        self.quick_open.filter(&self.workspace_files);
    }

    /// Close the Quick Open overlay and restore focus.
    pub fn close_quick_open(&mut self) {
        self.show_quick_open = false;
        self.quick_open.query.clear();
        self.quick_open.filtered_results.clear();
        self.quick_open.selected_index = 0;
        self.focused = self.saved_focus;
    }

    /// Handle a key press while the Quick Open overlay is focused.
    fn handle_quick_open_key(&mut self, key: KeyInput) -> WorkbenchAction {
        use vsedit_keycodes::KeyCode as KC;

        match key.key_code {
            KC::Escape => {
                self.close_quick_open();
                WorkbenchAction::None
            }
            KC::Enter => {
                let path = self.quick_open.selected_path().cloned();
                self.close_quick_open();
                if let Some(p) = path {
                    if let Ok(content) = std::fs::read_to_string(&p) {
                        self.open_file(&p, &content);
                    }
                }
                WorkbenchAction::None
            }
            KC::UpArrow => {
                self.quick_open.select_previous();
                WorkbenchAction::None
            }
            KC::DownArrow => {
                self.quick_open.select_next();
                WorkbenchAction::None
            }
            KC::Backspace => {
                self.quick_open.query.pop();
                let files = self.workspace_files.clone();
                self.quick_open.filter(&files);
                WorkbenchAction::None
            }
            other => {
                if !key.ctrl && !key.alt && !key.meta {
                    if let Some(ch) = key_code_to_char(other, key.shift) {
                        self.quick_open.query.push(ch);
                        let files = self.workspace_files.clone();
                        self.quick_open.filter(&files);
                    }
                }
                WorkbenchAction::None
            }
        }
    }

    /// Open the Go To Line input overlay.
    pub fn open_goto_line(&mut self) {
        self.saved_focus = self.focused;
        self.focused = FocusedPart::QuickInput;
        self.show_goto_line = true;
        self.goto_line_input.clear();
    }

    /// Close the Go To Line input overlay and restore focus.
    pub fn close_goto_line(&mut self) {
        self.show_goto_line = false;
        self.goto_line_input.clear();
        self.focused = self.saved_focus;
    }

    /// Handle a key press while the Go To Line input is focused.
    /// Returns `Some(line_number)` when the user confirms, `None` otherwise.
    pub fn handle_goto_line_key(&mut self, key: KeyInput) -> Option<u32> {
        use vsedit_keycodes::KeyCode as KC;

        match key.key_code {
            KC::Escape => {
                self.close_goto_line();
                None
            }
            KC::Enter => {
                let line: Option<u32> = self.goto_line_input.trim().parse().ok();
                self.close_goto_line();
                line
            }
            KC::Backspace => {
                self.goto_line_input.pop();
                None
            }
            other => {
                if !key.ctrl && !key.alt && !key.meta {
                    if let Some(ch) = key_code_to_char(other, key.shift) {
                        if ch.is_ascii_digit() {
                            self.goto_line_input.push(ch);
                        }
                    }
                }
                None
            }
        }
    }

    /// Render the Go To Line input overlay.
    fn render_goto_line(&self, frame: &mut Frame, area: Rect) {
        let width = 40u16.min(area.width);
        let height = 3u16;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + 1;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Go to Line ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::DarkGray));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let input_line = Line::from(vec![
            Span::styled(": ", Style::default().fg(Color::Yellow)),
            Span::raw(&self.goto_line_input),
        ]);
        frame.render_widget(Paragraph::new(input_line), inner);
    }

    /// Render the Quick Open overlay.
    fn render_quick_open(&self, frame: &mut Frame, area: Rect) {
        let width = (area.width * 3 / 5).max(20).min(area.width);
        let height = (area.height * 2 / 5).max(5).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + 1;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::DarkGray));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Input line
        let input_line = Line::from(vec![
            Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
            Span::raw(&self.quick_open.query),
        ]);
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(input_line), input_area);

        // Results list
        let list_start_y = inner.y + 1;
        let list_height = inner.height.saturating_sub(1) as usize;

        for (i, path) in self.quick_open.filtered_results.iter().take(list_height).enumerate() {
            let filename = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let dir = path.parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let style = if i == self.quick_open.selected_index {
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let mut spans = vec![Span::styled(filename, style)];
            if !dir.is_empty() {
                spans.push(Span::styled(
                    format!("  {}", dir),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let row_area = Rect::new(inner.x, list_start_y + i as u16, inner.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
        }
    }

    /// Set the list of workspace files available for Quick Open.
    pub fn set_workspace_files(&mut self, files: Vec<PathBuf>) {
        self.workspace_files = files;
    }

    /// Scan a workspace directory for files and populate [`workspace_files`].
    pub fn scan_workspace_files(&mut self, root: &Path) {
        self.workspace_files.clear();
        fn is_hidden(entry: &walkdir::DirEntry) -> bool {
            entry.file_name().to_str().is_some_and(|s| s.starts_with('.'))
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .take(10_000)
        {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                self.workspace_files.push(root.join(rel));
            }
        }
    }

    /// Switch the sidebar to the given panel, opening it if hidden.
    pub fn set_active_sidebar(&mut self, panel: ActiveSidebarPanel) {
        self.active_sidebar = panel;
        let view_id = match panel {
            ActiveSidebarPanel::Explorer => "workbench.view.explorer",
            ActiveSidebarPanel::Search => "workbench.view.search",
            ActiveSidebarPanel::SourceControl => "workbench.view.scm",
            ActiveSidebarPanel::Debug => "workbench.view.debug",
            ActiveSidebarPanel::Extensions => "workbench.view.extensions",
        };
        self.views.set_active_container(ViewContainerLocation::Sidebar, view_id);
        if !self.layout.is_part_visible(vsedit_wb_layout::Part::Sidebar) {
            self.layout.toggle_sidebar();
        }
        self.focused = FocusedPart::Sidebar;
    }

    /// Handle Up/Down/Enter keys when sidebar is focused.
    pub fn handle_sidebar_key(&mut self, key: &KeyInput) {
        use vsedit_keycodes::KeyCode as KC;
        match self.active_sidebar {
            ActiveSidebarPanel::Explorer => {
                if let Some(ref mut explorer) = self.explorer {
                    match key.key_code {
                        KC::UpArrow => explorer.move_up(),
                        KC::DownArrow => explorer.move_down(),
                        KC::Enter => { explorer.toggle_expand(); }
                        _ => {}
                    }
                }
            }
            ActiveSidebarPanel::Search => {
                match key.key_code {
                    KC::UpArrow => self.search_view.select_previous(),
                    KC::DownArrow => self.search_view.select_next(),
                    _ => {}
                }
            }
            ActiveSidebarPanel::SourceControl => {
                match key.key_code {
                    KC::UpArrow => {
                        self.scm_view.selected_index =
                            self.scm_view.selected_index.saturating_sub(1);
                    }
                    KC::DownArrow => {
                        let total: usize = self.scm_groups.iter()
                            .map(|g| g.visible_rows())
                            .sum();
                        if total > 0 {
                            self.scm_view.selected_index =
                                (self.scm_view.selected_index + 1).min(total - 1);
                        }
                    }
                    KC::Enter => {
                        // Toggle group expansion
                        let mut idx = 0usize;
                        for group in &mut self.scm_groups {
                            if idx == self.scm_view.selected_index {
                                group.toggle_expanded();
                                break;
                            }
                            idx += 1;
                            if group.is_expanded {
                                idx += group.changes.len();
                            }
                        }
                    }
                    _ => {}
                }
            }
            ActiveSidebarPanel::Debug => {
                match key.key_code {
                    KC::UpArrow => self.debug_view.select_previous(),
                    KC::DownArrow => self.debug_view.select_next(),
                    KC::Enter => self.debug_view.next_section(),
                    _ => {}
                }
            }
            ActiveSidebarPanel::Extensions => {
                // Stub — no interactive elements yet
            }
        }
    }

    /// Open a file as a new tab and set it as the active editor content.
    pub fn open_file(&mut self, path: &Path, content: &str) {
        self.tab_service.open_tab(Some(path.to_path_buf()), content);
        self.set_editor_content(content, Some(path.display().to_string()));
    }

    /// Return the content of the active tab, if any.
    pub fn get_active_content(&self) -> Option<&str> {
        self.tab_service
            .get_active_tab()
            .map(|t| t.content.as_str())
    }

    /// Sync workbench editor state from the currently active tab.
    fn sync_active_tab_to_editor(&mut self) {
        if let Some(tab) = self.tab_service.get_active_tab() {
            let content = tab.content.clone();
            let path = tab.file_path.as_ref().map(|p| p.display().to_string());
            self.is_modified = tab.is_modified;
            self.set_editor_content(&content, path);
        } else {
            self.editor_content = None;
            self.file_path = None;
            self.is_modified = false;
        }
    }

    /// Add a diagnostic problem to the problems panel.
    pub fn add_problem(
        &mut self,
        severity: ProblemSeverity,
        message: &str,
        file: &str,
        line: u32,
        col: u32,
    ) {
        self.problems_panel.problems.push(Problem::new(
            severity, message, "", file, line, col,
        ));
        self.update_problem_status();
    }

    /// Append text to an output channel, creating it if needed.
    pub fn append_output(&mut self, channel: &str, text: &str) {
        let idx = self
            .output_panel
            .channels
            .iter()
            .position(|c| c.name == channel);
        let idx = match idx {
            Some(i) => i,
            None => self.output_panel.create_channel(channel),
        };
        self.output_panel.append_line(idx, text);
    }

    /// Return counts of (errors, warnings, info) across all problems.
    pub fn get_problem_count(&self) -> (usize, usize, usize) {
        let errors = self.problems_panel.count_by_severity(ProblemSeverity::Error);
        let warnings = self.problems_panel.count_by_severity(ProblemSeverity::Warning);
        let info = self.problems_panel.count_by_severity(ProblemSeverity::Info);
        (errors, warnings, info)
    }

    /// Update the statusbar with current problem counts.
    fn update_problem_status(&mut self) {
        let (e, w, i) = self.get_problem_count();
        self.statusbar
            .update_item("statusbar.problems", &format!("✖ {} ⚠ {} ℹ {}", e, w, i));
    }

    /// Update activity bar badge counts from current state.
    pub fn update_activity_bar_badges(&mut self) {
        let scm_count: usize = self.scm_groups.iter().map(|g| g.changes.len()).sum();
        for item in &mut self.activity_bar_items {
            match item.id.as_str() {
                "workbench.view.scm" => item.set_badge(scm_count),
                _ => {}
            }
        }
    }

    /// Set the workspace folder for the title bar.
    pub fn set_workspace_folder(&mut self, folder: Option<String>) {
        self.workspace_folder = folder;
    }

    /// Get the computed title bar text.
    pub fn get_title_bar_text(&self) -> String {
        compute_title_bar(
            self.file_path.as_deref(),
            self.workspace_folder.as_deref(),
            self.is_modified,
        )
    }
}

/// Convert a command ID like "workbench.action.showCommands" to a label.
fn command_id_to_label(id: &str) -> String {
    let last_segment = id.rsplit('.').next().unwrap_or(id);
    let mut label = String::new();
    for (i, ch) in last_segment.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            label.push(' ');
        }
        if i == 0 {
            label.extend(ch.to_uppercase());
        } else {
            label.push(ch);
        }
    }
    label
}

/// Convert a KeyCode to a printable character, if applicable.
fn key_code_to_char(code: vsedit_keycodes::KeyCode, shift: bool) -> Option<char> {
    use vsedit_keycodes::KeyCode as KC;
    let ch = match code {
        KC::KeyA => 'a', KC::KeyB => 'b', KC::KeyC => 'c', KC::KeyD => 'd',
        KC::KeyE => 'e', KC::KeyF => 'f', KC::KeyG => 'g', KC::KeyH => 'h',
        KC::KeyI => 'i', KC::KeyJ => 'j', KC::KeyK => 'k', KC::KeyL => 'l',
        KC::KeyM => 'm', KC::KeyN => 'n', KC::KeyO => 'o', KC::KeyP => 'p',
        KC::KeyQ => 'q', KC::KeyR => 'r', KC::KeyS => 's', KC::KeyT => 't',
        KC::KeyU => 'u', KC::KeyV => 'v', KC::KeyW => 'w', KC::KeyX => 'x',
        KC::KeyY => 'y', KC::KeyZ => 'z',
        KC::Digit0 => '0', KC::Digit1 => '1', KC::Digit2 => '2', KC::Digit3 => '3',
        KC::Digit4 => '4', KC::Digit5 => '5', KC::Digit6 => '6', KC::Digit7 => '7',
        KC::Digit8 => '8', KC::Digit9 => '9',
        KC::Space => ' ',
        KC::Minus => '-', KC::Period => '.', KC::Comma => ',',
        _ => return None,
    };
    if shift && ch.is_ascii_alphabetic() {
        Some(ch.to_ascii_uppercase())
    } else {
        Some(ch)
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

    // -- Command palette tests ----------------------------------------------

    #[test]
    fn open_command_palette_sets_visible() {
        let mut wb = Workbench::new();
        assert!(!wb.show_command_palette);
        wb.execute_command("workbench.action.showCommands");
        assert!(wb.show_command_palette);
        assert_eq!(wb.focused, FocusedPart::CommandPalette);
        assert!(!wb.command_palette_items.is_empty());
    }

    #[test]
    fn command_palette_filter_narrows_items() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        let initial_count = wb.command_palette_items.len();
        assert!(initial_count > 0);

        wb.command_palette_input = "quit".to_string();
        wb.filter_palette_items();
        assert!(wb.command_palette_items.len() < initial_count);
        assert!(wb.command_palette_items.iter().any(|i| i.id.contains("quit")));
    }

    #[test]
    fn command_palette_escape_closes() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        assert!(wb.show_command_palette);

        let action = wb.handle_input(make_key(KeyCode::Escape, false, false));
        assert_eq!(action, WorkbenchAction::None);
        assert!(!wb.show_command_palette);
        assert_eq!(wb.focused, FocusedPart::Editor);
    }

    #[test]
    fn command_palette_enter_executes_selected() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        assert!(wb.show_command_palette);
        assert!(!wb.command_palette_items.is_empty());

        let first_cmd = wb.command_palette_items[0].id.clone();
        let action = wb.handle_input(make_key(KeyCode::Enter, false, false));
        assert_eq!(action, WorkbenchAction::ExecuteCommand(first_cmd));
        assert!(!wb.show_command_palette);
    }

    #[test]
    fn command_palette_up_down_navigation() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        assert_eq!(wb.command_palette_selected, 0);

        wb.handle_input(make_key(KeyCode::DownArrow, false, false));
        assert_eq!(wb.command_palette_selected, 1);

        wb.handle_input(make_key(KeyCode::UpArrow, false, false));
        assert_eq!(wb.command_palette_selected, 0);

        wb.handle_input(make_key(KeyCode::UpArrow, false, false));
        assert_eq!(wb.command_palette_selected, wb.command_palette_items.len() - 1);
    }

    #[test]
    fn command_palette_typing_filters() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        let initial_count = wb.command_palette_items.len();

        wb.handle_input(make_key(KeyCode::KeyT, false, false));
        assert_eq!(wb.command_palette_input, "t");
        assert!(wb.command_palette_items.len() <= initial_count);
    }

    #[test]
    fn command_palette_backspace_removes_char() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        wb.command_palette_input = "test".to_string();
        wb.filter_palette_items();

        wb.handle_input(make_key(KeyCode::Backspace, false, false));
        assert_eq!(wb.command_palette_input, "tes");
    }

    #[test]
    fn command_palette_keybinding_via_ctrl_shift_p() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyP, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.showCommands".to_string())
        );
    }

    #[test]
    fn new_commands_registered() {
        let wb = Workbench::new();
        assert!(wb.commands.has("workbench.action.showCommands"));
        assert!(wb.commands.has("workbench.action.quickOpen"));
        assert!(wb.commands.has("workbench.action.files.newUntitledFile"));
        assert!(wb.commands.has("workbench.action.files.openFile"));
        assert!(wb.commands.has("workbench.action.files.save"));
    }

    #[test]
    fn render_with_command_palette_does_not_panic() {
        let mut wb = Workbench::new();
        wb.open_command_palette();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    // -- Tab management -----------------------------------------------------

    #[test]
    fn open_file_creates_tab() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/foo.rs"), "fn main() {}");
        assert_eq!(wb.tab_service.tab_count(), 1);
        assert_eq!(wb.tab_service.get_active_tab().unwrap().title, "foo.rs");
    }

    #[test]
    fn open_file_sets_editor_content() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/bar.txt"), "hello\nworld");
        assert!(wb.editor_content.is_some());
        assert_eq!(wb.file_path.as_deref(), Some("/tmp/bar.txt"));
    }

    #[test]
    fn get_active_content_returns_tab_content() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/a.rs"), "content_a");
        assert_eq!(wb.get_active_content(), Some("content_a"));
    }

    #[test]
    fn next_editor_command_switches_tab() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/a.rs"), "a");
        wb.open_file(std::path::Path::new("/tmp/b.rs"), "b");
        wb.execute_command("workbench.action.nextEditor");
        assert_eq!(wb.tab_service.get_active_tab().unwrap().title, "a.rs");
    }

    #[test]
    fn previous_editor_command_switches_tab() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/a.rs"), "a");
        wb.open_file(std::path::Path::new("/tmp/b.rs"), "b");
        wb.execute_command("workbench.action.previousEditor");
        assert_eq!(wb.tab_service.get_active_tab().unwrap().title, "a.rs");
    }

    #[test]
    fn close_active_editor_command() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/a.rs"), "a");
        wb.open_file(std::path::Path::new("/tmp/b.rs"), "b");
        wb.execute_command("workbench.action.closeActiveEditor");
        assert_eq!(wb.tab_service.tab_count(), 1);
        assert_eq!(wb.tab_service.get_active_tab().unwrap().title, "a.rs");
    }

    #[test]
    fn render_with_tabs_does_not_panic() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/tmp/a.rs"), "line1\nline2");
        wb.open_file(std::path::Path::new("/tmp/b.rs"), "hello");
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    #[test]
    fn tab_keybinding_next_editor() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::Tab, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.nextEditor".to_string())
        );
    }

    #[test]
    fn tab_keybinding_previous_editor() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::Tab, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.previousEditor".to_string())
        );
    }

    #[test]
    fn tab_keybinding_close_editor() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyW, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.closeActiveEditor".to_string())
        );
    }

    // -- Bottom panel tests -------------------------------------------------

    #[test]
    fn default_active_panel_is_terminal() {
        let wb = Workbench::new();
        assert_eq!(wb.active_panel, ActivePanelView::Terminal);
    }

    #[test]
    fn toggle_terminal_focuses_panel() {
        let mut wb = Workbench::new();
        wb.execute_command("workbench.action.terminal.toggleTerminal");
        assert_eq!(wb.focused, FocusedPart::Panel);
        assert_eq!(wb.active_panel, ActivePanelView::Terminal);
        // Toggle again returns focus to editor.
        wb.execute_command("workbench.action.terminal.toggleTerminal");
        assert_eq!(wb.focused, FocusedPart::Editor);
    }

    #[test]
    fn focus_problems_panel_via_command() {
        let mut wb = Workbench::new();
        wb.execute_command("workbench.action.problems.focus");
        assert_eq!(wb.focused, FocusedPart::Panel);
        assert_eq!(wb.active_panel, ActivePanelView::Problems);
    }

    #[test]
    fn focus_output_panel_via_command() {
        let mut wb = Workbench::new();
        wb.execute_command("workbench.action.output.focus");
        assert_eq!(wb.focused, FocusedPart::Panel);
        assert_eq!(wb.active_panel, ActivePanelView::Output);
    }

    #[test]
    fn add_problem_and_get_count() {
        let mut wb = Workbench::new();
        wb.add_problem(ProblemSeverity::Error, "oops", "main.rs", 1, 1);
        wb.add_problem(ProblemSeverity::Warning, "warn", "lib.rs", 2, 5);
        wb.add_problem(ProblemSeverity::Info, "info", "lib.rs", 3, 1);
        assert_eq!(wb.get_problem_count(), (1, 1, 1));
        assert_eq!(wb.problems_panel.problems.len(), 3);
    }

    #[test]
    fn append_output_creates_channel() {
        let mut wb = Workbench::new();
        wb.append_output("Git", "commit abc");
        wb.append_output("Git", "push ok");
        wb.append_output("Rust", "building...");
        assert_eq!(wb.output_panel.channels.len(), 2);
        assert_eq!(wb.output_panel.channels[0].content.len(), 2);
        assert_eq!(wb.output_panel.channels[1].content.len(), 1);
    }

    #[test]
    fn panel_arrow_keys_switch_tabs() {
        let mut wb = Workbench::new();
        wb.focused = FocusedPart::Panel;
        assert_eq!(wb.active_panel, ActivePanelView::Terminal);
        wb.handle_input(make_key(KeyCode::RightArrow, false, false));
        assert_eq!(wb.active_panel, ActivePanelView::Problems);
        wb.handle_input(make_key(KeyCode::LeftArrow, false, false));
        assert_eq!(wb.active_panel, ActivePanelView::Terminal);
        // Left from Terminal wraps to DebugConsole.
        wb.handle_input(make_key(KeyCode::LeftArrow, false, false));
        assert_eq!(wb.active_panel, ActivePanelView::DebugConsole);
    }

    #[test]
    fn panel_commands_registered() {
        let wb = Workbench::new();
        assert!(wb.commands.has("workbench.action.terminal.toggleTerminal"));
        assert!(wb.commands.has("workbench.action.problems.focus"));
        assert!(wb.commands.has("workbench.action.output.focus"));
        assert!(wb.commands.has("workbench.action.panel.nextTab"));
        assert!(wb.commands.has("workbench.action.panel.previousTab"));
    }

    #[test]
    fn render_with_panel_content_does_not_panic() {
        let mut wb = Workbench::new();
        wb.add_problem(ProblemSeverity::Error, "e", "a.rs", 1, 1);
        wb.append_output("Test", "line");
        wb.terminal_view.add_tab("bash");

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        // Render each panel view to verify none panic.
        for panel in ActivePanelView::all() {
            wb.active_panel = *panel;
            terminal.draw(|frame| wb.render(frame)).unwrap();
        }
    }

    #[test]
    fn keybinding_ctrl_backtick_toggles_terminal() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::Backquote, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand(
                "workbench.action.terminal.toggleTerminal".to_string()
            )
        );
    }

    #[test]
    fn keybinding_ctrl_shift_m_focuses_problems() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyM, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand(
                "workbench.action.problems.focus".to_string()
            )
        );
    }

    #[test]
    fn keybinding_ctrl_shift_u_focuses_output() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyU, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand(
                "workbench.action.output.focus".to_string()
            )
        );
    }

    // -- Sidebar panel tests ------------------------------------------------

    #[test]
    fn default_active_sidebar_is_explorer() {
        let wb = Workbench::new();
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::Explorer);
    }

    #[test]
    fn set_active_sidebar_switches_panel() {
        let mut wb = Workbench::new();
        wb.set_active_sidebar(ActiveSidebarPanel::Search);
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::Search);
        assert_eq!(wb.focused, FocusedPart::Sidebar);
        assert_eq!(
            wb.views.get_active_container(ViewContainerLocation::Sidebar),
            Some("workbench.view.search"),
        );
    }

    #[test]
    fn set_active_sidebar_opens_hidden_sidebar() {
        let mut wb = Workbench::new();
        wb.layout.toggle_sidebar();
        assert!(!wb.layout.is_part_visible(Part::Sidebar));
        wb.set_active_sidebar(ActiveSidebarPanel::Debug);
        assert!(wb.layout.is_part_visible(Part::Sidebar));
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::Debug);
    }

    #[test]
    fn sidebar_keybinding_ctrl_shift_e() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyE, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.view.explorer".to_string())
        );
    }

    #[test]
    fn sidebar_keybinding_ctrl_shift_f() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyF, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.view.search".to_string())
        );
    }

    #[test]
    fn sidebar_keybinding_ctrl_shift_g() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyG, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.view.scm".to_string())
        );
    }

    #[test]
    fn sidebar_keybinding_ctrl_shift_d() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyD, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.view.debug".to_string())
        );
    }

    #[test]
    fn sidebar_keybinding_ctrl_shift_x() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::KeyX, true, true));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.view.extensions".to_string())
        );
    }

    #[test]
    fn sidebar_commands_registered() {
        let wb = Workbench::new();
        assert!(wb.commands.has("workbench.view.explorer"));
        assert!(wb.commands.has("workbench.view.search"));
        assert!(wb.commands.has("workbench.view.scm"));
        assert!(wb.commands.has("workbench.view.debug"));
        assert!(wb.commands.has("workbench.view.extensions"));
    }

    #[test]
    fn render_each_sidebar_panel_no_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for panel in [
            ActiveSidebarPanel::Explorer,
            ActiveSidebarPanel::Search,
            ActiveSidebarPanel::SourceControl,
            ActiveSidebarPanel::Debug,
            ActiveSidebarPanel::Extensions,
        ] {
            let mut wb = Workbench::new();
            wb.active_sidebar = panel;
            terminal
                .draw(|frame| {
                    wb.render(frame);
                })
                .unwrap();
        }
    }

    #[test]
    fn sidebar_key_routing_debug_view() {
        let mut wb = Workbench::new();
        wb.set_active_sidebar(ActiveSidebarPanel::Debug);
        wb.debug_view.variables.push(
            vsedit_debug_view::DebugVariable::new("x", "42", "i32"),
        );
        wb.debug_view.variables.push(
            vsedit_debug_view::DebugVariable::new("y", "10", "i32"),
        );
        assert_eq!(wb.debug_view.selected_index, 0);
        wb.handle_sidebar_key(&KeyInput {
            key_code: KeyCode::DownArrow,
            ctrl: false, shift: false, alt: false, meta: false,
        });
        assert_eq!(wb.debug_view.selected_index, 1);
        wb.handle_sidebar_key(&KeyInput {
            key_code: KeyCode::UpArrow,
            ctrl: false, shift: false, alt: false, meta: false,
        });
        assert_eq!(wb.debug_view.selected_index, 0);
    }

    #[test]
    fn sidebar_key_routing_search_view() {
        let mut wb = Workbench::new();
        wb.set_active_sidebar(ActiveSidebarPanel::Search);
        wb.handle_sidebar_key(&KeyInput {
            key_code: KeyCode::DownArrow,
            ctrl: false, shift: false, alt: false, meta: false,
        });
        wb.handle_sidebar_key(&KeyInput {
            key_code: KeyCode::UpArrow,
            ctrl: false, shift: false, alt: false, meta: false,
        });
    }

    #[test]
    fn scm_badge_in_activity_bar() {
        use vsedit_scm_view::{ScmFileChange, ScmFileStatus, ScmGroup};
        let mut wb = Workbench::new();
        let mut group = ScmGroup::new("changes", "Changes");
        group.add_change(ScmFileChange::new("a.rs", ScmFileStatus::Modified));
        group.add_change(ScmFileChange::new("b.rs", ScmFileStatus::Added));
        wb.scm_groups = vec![group];
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                wb.render(frame);
            })
            .unwrap();
    }

    // -- Editor group management tests --------------------------------------

    #[test]
    fn editor_group_manager_starts_with_single_group() {
        let mgr = EditorGroupManager::new();
        assert_eq!(mgr.group_count(), 1);
        assert_eq!(mgr.layout, EditorGroupLayout::Single);
        assert!(mgr.active_group().unwrap().is_empty());
    }

    #[test]
    fn editor_group_add_and_close_tab() {
        let mut group = EditorGroup::new(0);
        group.add_tab(EditorGroupTab {
            title: "main.rs".into(),
            file_path: Some("/src/main.rs".into()),
            content: "fn main() {}".into(),
            is_modified: false,
        });
        group.add_tab(EditorGroupTab {
            title: "lib.rs".into(),
            file_path: Some("/src/lib.rs".into()),
            content: "pub mod foo;".into(),
            is_modified: true,
        });
        assert_eq!(group.tabs.len(), 2);
        assert_eq!(group.active_tab_idx, 1);
        assert_eq!(group.active_tab().unwrap().title, "lib.rs");

        let closed = group.close_tab(1).unwrap();
        assert_eq!(closed.title, "lib.rs");
        assert_eq!(group.tabs.len(), 1);
        assert_eq!(group.active_tab_idx, 0);
    }

    #[test]
    fn split_editor_creates_new_group() {
        let mut mgr = EditorGroupManager::new();
        mgr.active_group_mut().unwrap().add_tab(EditorGroupTab {
            title: "test.rs".into(),
            file_path: None,
            content: "// test".into(),
            is_modified: false,
        });
        let new_id = mgr.split_editor(SplitDirection::Right);
        assert_eq!(mgr.group_count(), 2);
        assert_eq!(mgr.active_group_id, new_id);
        assert_eq!(mgr.active_group().unwrap().tabs.len(), 1);
        assert_eq!(mgr.active_group().unwrap().tabs[0].title, "test.rs");
        assert!(matches!(mgr.layout, EditorGroupLayout::Horizontal(_)));
    }

    #[test]
    fn split_editor_vertical() {
        let mut mgr = EditorGroupManager::new();
        mgr.split_editor(SplitDirection::Down);
        assert!(matches!(mgr.layout, EditorGroupLayout::Vertical(_)));
    }

    #[test]
    fn close_group_redistributes_tabs() {
        let mut mgr = EditorGroupManager::new();
        mgr.active_group_mut().unwrap().add_tab(EditorGroupTab {
            title: "a.rs".into(),
            file_path: None,
            content: "a".into(),
            is_modified: false,
        });
        let new_id = mgr.split_editor(SplitDirection::Right);
        mgr.active_group_mut().unwrap().add_tab(EditorGroupTab {
            title: "b.rs".into(),
            file_path: None,
            content: "b".into(),
            is_modified: false,
        });
        assert!(mgr.close_group(new_id));
        assert_eq!(mgr.group_count(), 1);
        assert!(mgr.active_group().unwrap().tabs.len() >= 2);
        assert_eq!(mgr.layout, EditorGroupLayout::Single);
    }

    #[test]
    fn close_last_group_fails() {
        let mut mgr = EditorGroupManager::new();
        assert!(!mgr.close_group(0));
    }

    #[test]
    fn move_editor_to_group() {
        let mut mgr = EditorGroupManager::new();
        mgr.active_group_mut().unwrap().add_tab(EditorGroupTab {
            title: "file.rs".into(),
            file_path: None,
            content: "code".into(),
            is_modified: false,
        });
        let first_id = mgr.active_group_id;
        let second_id = mgr.split_editor(SplitDirection::Right);
        assert!(mgr.move_editor_to_group(first_id, second_id, 0));
        assert!(mgr.get_group(first_id).unwrap().is_empty());
    }

    #[test]
    fn focus_group() {
        let mut mgr = EditorGroupManager::new();
        let first_id = mgr.active_group_id;
        let second_id = mgr.split_editor(SplitDirection::Right);
        assert_eq!(mgr.active_group_id, second_id);
        assert!(mgr.focus_group(first_id));
        assert_eq!(mgr.active_group_id, first_id);
        assert!(!mgr.focus_group(999));
    }

    // -- Activity bar item tests --------------------------------------------

    #[test]
    fn activity_bar_item_badge() {
        let mut item = ActivityBarItem::new("test", "Test", "🔧");
        assert_eq!(item.display_text(), "🔧");
        item.set_badge(5);
        assert_eq!(item.display_text(), "🔧5");
        item.set_badge(0);
        assert_eq!(item.display_text(), "🔧");
    }

    #[test]
    fn default_activity_bar_items_count() {
        let items = default_activity_bar_items();
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].id, "workbench.view.explorer");
        assert_eq!(items[1].icon_char, "🔍");
        assert_eq!(items[2].icon_char, "🔀");
        assert_eq!(items[3].icon_char, "🐛");
        assert_eq!(items[4].icon_char, "📦");
    }

    // -- Breadcrumb tests ---------------------------------------------------

    #[test]
    fn compute_breadcrumbs_file_path() {
        let crumbs = compute_breadcrumbs("src/main.rs", &[]);
        assert!(crumbs.len() >= 2);
        assert_eq!(crumbs.last().unwrap().label, "main.rs");
        assert_eq!(crumbs.last().unwrap().kind, BreadcrumbKind::File);
        assert_eq!(crumbs[0].kind, BreadcrumbKind::Folder);
    }

    #[test]
    fn compute_breadcrumbs_with_symbols() {
        let crumbs = compute_breadcrumbs("src/lib.rs", &["MyStruct".to_string(), "method".to_string()]);
        let symbols: Vec<_> = crumbs.iter().filter(|c| c.kind == BreadcrumbKind::Symbol).collect();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].label, "MyStruct");
    }

    #[test]
    fn compute_breadcrumbs_empty_path() {
        let crumbs = compute_breadcrumbs("file.txt", &[]);
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "file.txt");
        assert_eq!(crumbs[0].kind, BreadcrumbKind::File);
    }

    // -- Title bar tests ----------------------------------------------------

    #[test]
    fn title_bar_no_file() {
        assert_eq!(compute_title_bar(None, None, false), "vsedit");
    }

    #[test]
    fn title_bar_with_file_and_folder() {
        let text = compute_title_bar(Some("/src/main.rs"), Some("myproject"), false);
        assert!(text.contains("main.rs"));
        assert!(text.contains("myproject"));
        assert!(text.contains("vsedit"));
        assert!(!text.contains("●"));
    }

    #[test]
    fn title_bar_dirty_indicator() {
        let text = compute_title_bar(Some("/src/main.rs"), None, true);
        assert!(text.contains("●"));
    }

    // -- Keybinding tests for new commands ----------------------------------

    #[test]
    fn keybinding_ctrl_backslash_splits_editor() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::Backslash, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.splitEditor".to_string())
        );
    }

    #[test]
    fn keybinding_ctrl_1_focuses_first_group() {
        let mut wb = Workbench::new();
        let action = wb.handle_input(make_key(KeyCode::Digit1, true, false));
        assert_eq!(
            action,
            WorkbenchAction::ExecuteCommand("workbench.action.focusFirstEditorGroup".to_string())
        );
    }

    // -- Resize sidebar tests -----------------------------------------------

    #[test]
    fn decrease_sidebar_width() {
        let mut wb = Workbench::new();
        let initial = wb.layout.get_sidebar_width();
        wb.execute_command("workbench.action.decreaseSidebarWidth");
        assert_eq!(wb.layout.get_sidebar_width(), initial - 2);
    }

    #[test]
    fn increase_sidebar_width() {
        let mut wb = Workbench::new();
        let initial = wb.layout.get_sidebar_width();
        wb.execute_command("workbench.action.increaseSidebarWidth");
        assert_eq!(wb.layout.get_sidebar_width(), initial + 2);
    }

    #[test]
    fn sidebar_width_minimum_clamp() {
        let mut wb = Workbench::new();
        wb.layout.set_sidebar_width(10);
        wb.execute_command("workbench.action.decreaseSidebarWidth");
        assert_eq!(wb.layout.get_sidebar_width(), 10);
    }

    // -- Render with new features -------------------------------------------

    #[test]
    fn render_with_breadcrumbs_and_minimap() {
        let mut wb = Workbench::new();
        wb.open_file(std::path::Path::new("/src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n");
        assert!(!wb.breadcrumbs.is_empty());
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| wb.render(frame)).unwrap();
    }

    #[test]
    fn render_with_workspace_folder_title() {
        let mut wb = Workbench::new();
        wb.set_workspace_folder(Some("myproject".into()));
        wb.open_file(std::path::Path::new("/src/main.rs"), "code");
        let title = wb.get_title_bar_text();
        assert!(title.contains("myproject"));
        assert!(title.contains("main.rs"));
    }

    #[test]
    fn update_activity_bar_badges_from_scm() {
        use vsedit_scm_view::{ScmFileChange, ScmFileStatus, ScmGroup};
        let mut wb = Workbench::new();
        let mut group = ScmGroup::new("changes", "Changes");
        group.add_change(ScmFileChange::new("a.rs", ScmFileStatus::Modified));
        group.add_change(ScmFileChange::new("b.rs", ScmFileStatus::Added));
        wb.scm_groups = vec![group];
        wb.update_activity_bar_badges();
        let scm_item = wb.activity_bar_items.iter().find(|i| i.id == "workbench.view.scm").unwrap();
        assert_eq!(scm_item.badge, Some(2));
    }
}
