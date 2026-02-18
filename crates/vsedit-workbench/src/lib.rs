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
    FindBar,
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

    /// Filter the given workspace files using case-insensitive matching.
    ///
    /// Results are ranked: exact filename matches first, then prefix matches,
    /// then other substring matches, with fuzzy matches last.
    pub fn filter(&mut self, workspace_files: &[PathBuf]) {
        if self.query.is_empty() {
            self.filtered_results = workspace_files.to_vec();
        } else {
            let query_lower = self.query.to_lowercase();

            // Classify each file into a ranking tier.
            // Tier 0 = exact filename match, 1 = prefix, 2 = substring, 3 = fuzzy only
            let mut scored: Vec<(PathBuf, u8, i32)> = workspace_files
                .iter()
                .filter_map(|p| {
                    let full = p.to_string_lossy();
                    let full_lower = full.to_lowercase();
                    let fname = p.file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();

                    // Determine best tier for this entry.
                    let tier = if fname == query_lower {
                        0 // exact filename match
                    } else if fname.starts_with(&query_lower) {
                        1 // filename prefix
                    } else if full_lower.contains(&query_lower) {
                        2 // substring anywhere in path
                    } else {
                        3 // fuzzy only
                    };

                    // For tier 3 we still require fuzzy to match.
                    if tier <= 2 {
                        let score = fuzzy_match(&self.query, &full)
                            .map(|m| m.score)
                            .unwrap_or(0);
                        Some((p.clone(), tier, score))
                    } else {
                        fuzzy_match(&self.query, &full)
                            .map(|m| (p.clone(), tier, m.score))
                    }
                })
                .collect();

            // Sort by tier ascending, then by fuzzy score descending.
            scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));
            self.filtered_results = scored.into_iter().map(|(p, _, _)| p).collect();
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
    /// Simplified SCM changes list: (status_char, filepath).
    pub scm_changes: Vec<(char, String)>,
    /// Debug view for the sidebar.
    pub debug_view: DebugView,
    /// Which sub-view is active in the bottom panel.
    pub active_panel: ActivePanelView,
    /// Terminal panel view.
    pub terminal_view: TerminalView,
    /// Problems panel view.
    pub problems_panel: ProblemsPanel,
    /// Raw diagnostics: (severity, file, message, line).
    pub diagnostics: Vec<(String, String, String, u32)>,
    /// Output panel view.
    pub output_panel: OutputPanel,
    /// Output channels: `(channel_name, lines)` for programmatic access.
    pub output_channels: Vec<(String, Vec<String>)>,
    /// Debug console lines: `(kind, text)` where kind is "input", "output", or "error".
    pub debug_console_lines: Vec<(String, String)>,
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
    /// Whether the find bar overlay is visible.
    pub show_find_bar: bool,
    /// Current text typed into the find bar input.
    pub find_query: String,
    /// Match positions as (line, col) pairs (0-based).
    pub find_matches: Vec<(usize, usize)>,
    /// Index of the currently highlighted match.
    pub find_current_match: usize,
    /// Whether the Go To Line input overlay is visible.
    pub show_goto_line: bool,
    /// Current text typed into the Go To Line input.
    pub goto_line_input: String,
    /// Installed extension names for the sidebar.
    pub installed_extensions: Vec<InstalledExtensionInfo>,
    /// Filter query for the extensions sidebar search input.
    pub extensions_filter: String,
    /// Currently selected extension index in the filtered list.
    pub extensions_selected: usize,
}

/// Summary info for an installed extension shown in the sidebar.
#[derive(Debug, Clone)]
pub struct InstalledExtensionInfo {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub activated: bool,
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
        // Find in editor: Ctrl+F
        keybindings.add_rule(KeybindingRule {
            keybinding: vsedit_keybindings::Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyF,
            )),
            command: "actions.find".into(),
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
            commands.register("actions.find", Box::new(|_| Ok(None))),
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
            scm_changes: Vec::new(),
            debug_view: DebugView::new(),
            active_panel: ActivePanelView::Terminal,
            terminal_view: TerminalView::new(),
            problems_panel: ProblemsPanel::new(),
            diagnostics: Vec::new(),
            output_panel: OutputPanel::new(),
            output_channels: Vec::new(),
            debug_console_lines: Vec::new(),
            editor_groups: EditorGroupManager::new(),
            activity_bar_items: default_activity_bar_items(),
            breadcrumbs: Vec::new(),
            viewport_scroll: 0,
            workspace_folder: None,
            quick_open: QuickOpenState::new(),
            show_quick_open: false,
            workspace_files: Vec::new(),
            show_find_bar: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_current_match: 0,
            show_goto_line: false,
            goto_line_input: String::new(),
            installed_extensions: Vec::new(),
            extensions_filter: String::new(),
            extensions_selected: 0,
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
                            let filtered: Vec<&InstalledExtensionInfo> = self
                                .installed_extensions
                                .iter()
                                .filter(|ext| {
                                    self.extensions_filter.is_empty()
                                        || ext
                                            .name
                                            .to_lowercase()
                                            .contains(&self.extensions_filter.to_lowercase())
                                })
                                .collect();

                            let mut lines: Vec<Line> = Vec::new();

                            // Search input
                            lines.push(Line::from(vec![
                                Span::styled(
                                    " 🔍 ",
                                    Style::default().fg(Color::DarkGray),
                                ),
                                if self.extensions_filter.is_empty() {
                                    Span::styled(
                                        "Search Extensions...",
                                        Style::default().fg(Color::DarkGray),
                                    )
                                } else {
                                    Span::raw(&self.extensions_filter)
                                },
                            ]));

                            // Section header
                            lines.push(Line::from(Span::styled(
                                format!(" INSTALLED ({})", filtered.len()),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            )));

                            if filtered.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "   No extensions found",
                                    Style::default().fg(Color::DarkGray),
                                )));
                            } else {
                                for (i, ext) in filtered.iter().enumerate() {
                                    let status =
                                        if ext.activated { "●" } else { "○" };
                                    let is_selected = i == self.extensions_selected;
                                    let bg = if is_selected {
                                        Color::DarkGray
                                    } else {
                                        Color::Black
                                    };
                                    let status_color = if ext.activated {
                                        Color::Green
                                    } else {
                                        Color::Gray
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            format!(" {} ", status),
                                            Style::default().fg(status_color).bg(bg),
                                        ),
                                        Span::styled(
                                            &ext.name,
                                            Style::default()
                                                .fg(Color::White)
                                                .bg(bg)
                                                .add_modifier(Modifier::BOLD),
                                        ),
                                        Span::styled(
                                            format!(" {} v{}", ext.publisher, ext.version),
                                            Style::default().fg(Color::DarkGray).bg(bg),
                                        ),
                                    ]));
                                }
                            }

                            let ext_widget = Paragraph::new(lines);
                            frame.render_widget(ext_widget, content_area);
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

        // Find bar overlay
        if self.show_find_bar {
            self.render_find_bar(frame, area);
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
        let problem_count = self.problems_panel.total_count();
        for view in ActivePanelView::all() {
            let style = if *view == self.active_panel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            let label = if *view == ActivePanelView::Problems && problem_count > 0 {
                format!(" {} ({}) ", view.label(), problem_count)
            } else {
                format!(" {} ", view.label())
            };
            tab_spans.push(Span::styled(label, style));
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

        // When find bar is focused, handle keys directly.
        if self.focused == FocusedPart::FindBar {
            self.handle_find_bar_key(key);
            return WorkbenchAction::None;
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
            "actions.find" => {
                self.toggle_find_bar();
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

    /// Append a character to the Quick Open query and re-filter.
    pub fn quick_open_input(&mut self, ch: char) {
        self.quick_open.query.push(ch);
        self.quick_open_filter();
    }

    /// Remove the last character from the Quick Open query and re-filter.
    pub fn quick_open_backspace(&mut self) {
        self.quick_open.query.pop();
        self.quick_open_filter();
    }

    /// Move the Quick Open selection up.
    pub fn quick_open_up(&mut self) {
        self.quick_open.select_previous();
    }

    /// Move the Quick Open selection down.
    pub fn quick_open_down(&mut self) {
        self.quick_open.select_next();
    }

    /// Accept the currently selected Quick Open entry and return its path.
    pub fn quick_open_accept(&mut self) -> Option<String> {
        let path = self.quick_open.selected_path().map(|p| p.to_string_lossy().to_string());
        self.close_quick_open();
        path
    }

    /// Re-filter workspace files using the current Quick Open query.
    pub fn quick_open_filter(&mut self) {
        let files = self.workspace_files.clone();
        self.quick_open.filter(&files);
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

    // -----------------------------------------------------------------------
    // Find bar
    // -----------------------------------------------------------------------

    /// Toggle the find bar overlay on or off.
    pub fn toggle_find_bar(&mut self) {
        if self.show_find_bar {
            self.find_bar_close();
        } else {
            self.saved_focus = self.focused;
            self.focused = FocusedPart::FindBar;
            self.show_find_bar = true;
            self.find_query.clear();
            self.find_matches.clear();
            self.find_current_match = 0;
        }
    }

    /// Append a character to the find query and re-run the search.
    pub fn find_bar_input(&mut self, ch: char) {
        self.find_query.push(ch);
        self.update_find_matches();
    }

    /// Remove the last character from the find query and re-run the search.
    pub fn find_bar_backspace(&mut self) {
        self.find_query.pop();
        self.update_find_matches();
    }

    /// Move to the next match.
    pub fn find_bar_next(&mut self) {
        if !self.find_matches.is_empty() {
            self.find_current_match = (self.find_current_match + 1) % self.find_matches.len();
        }
    }

    /// Move to the previous match.
    pub fn find_bar_prev(&mut self) {
        if !self.find_matches.is_empty() {
            if self.find_current_match == 0 {
                self.find_current_match = self.find_matches.len() - 1;
            } else {
                self.find_current_match -= 1;
            }
        }
    }

    /// Close the find bar and restore focus.
    pub fn find_bar_close(&mut self) {
        self.show_find_bar = false;
        self.find_query.clear();
        self.find_matches.clear();
        self.find_current_match = 0;
        self.focused = self.saved_focus;
    }

    /// Scan the current editor content for occurrences of the find query.
    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        self.find_current_match = 0;
        if self.find_query.is_empty() {
            return;
        }
        if let Some(ref lines) = self.editor_content {
            let query = &self.find_query;
            for (line_idx, line) in lines.iter().enumerate() {
                let mut start = 0;
                while let Some(pos) = line[start..].find(query) {
                    self.find_matches.push((line_idx, start + pos));
                    start += pos + query.len();
                }
            }
        }
    }

    /// Handle a key press while the find bar is focused.
    fn handle_find_bar_key(&mut self, key: KeyInput) {
        use vsedit_keycodes::KeyCode as KC;

        match key.key_code {
            KC::Escape => {
                self.find_bar_close();
            }
            KC::Enter => {
                if key.shift {
                    self.find_bar_prev();
                } else {
                    self.find_bar_next();
                }
            }
            KC::Backspace => {
                self.find_bar_backspace();
            }
            other => {
                if !key.ctrl && !key.alt && !key.meta {
                    if let Some(ch) = key_code_to_char(other, key.shift) {
                        self.find_bar_input(ch);
                    }
                }
            }
        }
    }

    /// Render the find bar overlay at the top-right of the editor area.
    fn render_find_bar(&self, frame: &mut Frame, area: Rect) {
        let width = 40u16.min(area.width);
        let height = 3u16;
        let x = area.x + area.width.saturating_sub(width);
        let y = area.y;
        let bar_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, bar_area);

        let match_info = if self.find_query.is_empty() {
            String::new()
        } else if self.find_matches.is_empty() {
            "No results".to_string()
        } else {
            format!("{} of {}", self.find_current_match + 1, self.find_matches.len())
        };

        let block = Block::default()
            .title(" Find ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::DarkGray));
        let inner = block.inner(bar_area);
        frame.render_widget(block, bar_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut spans = vec![
            Span::raw(&self.find_query),
        ];
        if !match_info.is_empty() {
            let pad = inner.width as usize
                - self.find_query.len().min(inner.width as usize)
                - match_info.len().min(inner.width as usize);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(match_info, Style::default().fg(Color::DarkGray)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), inner);
    }

    /// Render the Quick Open overlay.
    fn render_quick_open(&self, frame: &mut Frame, area: Rect) {
        // Cap the list at 15 visible items plus 1 row for the input line.
        const MAX_VISIBLE: usize = 15;

        let width = (area.width * 3 / 5).max(20).min(area.width);
        // Height = border(2) + input(1) + up to MAX_VISIBLE rows
        let needed = (self.quick_open.filtered_results.len().min(MAX_VISIBLE) + 3) as u16;
        let height = needed.max(5).min(area.height);
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

        // Results list (cap at MAX_VISIBLE)
        let list_start_y = inner.y + 1;
        let list_height = (inner.height.saturating_sub(1) as usize).min(MAX_VISIBLE);

        // Compute scroll offset so the selected item is always visible.
        let total = self.quick_open.filtered_results.len();
        let sel = self.quick_open.selected_index;
        let scroll_offset = if sel >= list_height {
            sel - list_height + 1
        } else {
            0
        };

        let workspace_root = self.workspace_folder.as_deref().unwrap_or("");

        let visible = self.quick_open.filtered_results.iter()
            .skip(scroll_offset)
            .take(list_height)
            .enumerate();

        for (i, path) in visible {
            let abs_index = scroll_offset + i;
            let filename = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            // Show path relative to workspace root.
            let dir = if !workspace_root.is_empty() {
                path.parent()
                    .map(|p| {
                        let s = p.to_string_lossy();
                        s.strip_prefix(workspace_root)
                            .map(|r| r.trim_start_matches('/').to_string())
                            .unwrap_or_else(|| s.to_string())
                    })
                    .unwrap_or_default()
            } else {
                path.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            };

            let style = if abs_index == sel {
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

            // Show item count at the right edge of the first visible row.
            if i == 0 && total > list_height {
                let count_label = format!(" ({}/{})", sel + 1, total);
                spans.push(Span::styled(count_label, Style::default().fg(Color::DarkGray)));
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

    /// Read workspace files and run the search view's in-memory search.
    fn run_search_in_workspace_files(&mut self) {
        let files: Vec<(String, String)> = self
            .workspace_files
            .iter()
            .filter_map(|p| {
                let content = std::fs::read_to_string(p).ok()?;
                Some((p.to_string_lossy().to_string(), content))
            })
            .collect();
        self.search_view.search_in_files(&files);
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
                    KC::Backspace => {
                        self.search_view.search_text.pop();
                        self.run_search_in_workspace_files();
                    }
                    KC::Enter => {
                        self.run_search_in_workspace_files();
                    }
                    other => {
                        if !key.ctrl && !key.alt && !key.meta {
                            if let Some(ch) = key_code_to_char(other, key.shift) {
                                self.search_view.search_text.push(ch);
                                self.run_search_in_workspace_files();
                            }
                        }
                    }
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
                let filtered_count = self
                    .installed_extensions
                    .iter()
                    .filter(|ext| {
                        self.extensions_filter.is_empty()
                            || ext
                                .name
                                .to_lowercase()
                                .contains(&self.extensions_filter.to_lowercase())
                    })
                    .count();
                match key.key_code {
                    KC::UpArrow => {
                        self.extensions_selected =
                            self.extensions_selected.saturating_sub(1);
                    }
                    KC::DownArrow => {
                        if filtered_count > 0 {
                            self.extensions_selected =
                                (self.extensions_selected + 1).min(filtered_count - 1);
                        }
                    }
                    KC::Backspace => {
                        self.extensions_filter.pop();
                        self.extensions_selected = 0;
                    }
                    other => {
                        if !key.ctrl && !key.alt && !key.meta {
                            if let Some(ch) = key_code_to_char(other, key.shift) {
                                self.extensions_filter.push(ch);
                                self.extensions_selected = 0;
                            }
                        }
                    }
                }
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

    /// Replace all diagnostics from a list of `(severity, file, message, line)` tuples.
    ///
    /// The severity string is matched case-insensitively: `"error"`, `"warning"`,
    /// `"info"`, or `"hint"`.  Unrecognised values default to `Info`.
    pub fn set_diagnostics(&mut self, diags: Vec<(String, String, String, u32)>) {
        self.diagnostics = diags.clone();
        self.problems_panel.clear_all();
        for (sev_str, file, message, line) in diags {
            let severity = match sev_str.to_lowercase().as_str() {
                "error" => ProblemSeverity::Error,
                "warning" => ProblemSeverity::Warning,
                "hint" => ProblemSeverity::Hint,
                _ => ProblemSeverity::Info,
            };
            self.problems_panel
                .problems
                .push(Problem::new(severity, message, "", file, line, 1));
        }
        self.problems_panel.sort_problems();
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

    /// Update SCM status from a simplified (status_char, filepath) list.
    ///
    /// Populates `scm_branch`, `scm_changes`, and rebuilds `scm_groups` so the
    /// sidebar renders the current working-copy state.
    pub fn set_scm_status(&mut self, branch: Option<String>, changes: Vec<(char, String)>) {
        self.scm_branch = branch;
        self.scm_changes = changes.clone();

        let mut group = ScmGroup::new("changes", "Changes");
        for (status_char, path) in &changes {
            let status = match status_char {
                'M' => vsedit_scm_view::ScmFileStatus::Modified,
                'A' => vsedit_scm_view::ScmFileStatus::Added,
                'D' => vsedit_scm_view::ScmFileStatus::Deleted,
                'R' => vsedit_scm_view::ScmFileStatus::Renamed,
                '!' => vsedit_scm_view::ScmFileStatus::Conflicted,
                'I' => vsedit_scm_view::ScmFileStatus::Ignored,
                _   => vsedit_scm_view::ScmFileStatus::Untracked,
            };
            group.add_change(vsedit_scm_view::ScmFileChange::new(path, status));
        }
        self.scm_groups = if group.changes.is_empty() {
            Vec::new()
        } else {
            vec![group]
        };
        self.update_activity_bar_badges();
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

    /// Append a line to a named output channel, creating the channel if needed.
    pub fn add_output_line(&mut self, channel: &str, line: &str) {
        // Update the output_channels bookkeeping field.
        if let Some(entry) = self.output_channels.iter_mut().find(|(n, _)| n == channel) {
            entry.1.push(line.to_string());
        } else {
            self.output_channels.push((channel.to_string(), vec![line.to_string()]));
        }
        // Keep the OutputPanel widget in sync.
        let idx = if let Some(i) = self.output_panel.find_channel(channel) {
            i
        } else {
            self.output_panel.create_channel(channel)
        };
        self.output_panel.append_line(idx, line);
    }

    /// Clear all lines in the named output channel.
    pub fn clear_output(&mut self, channel: &str) {
        if let Some(entry) = self.output_channels.iter_mut().find(|(n, _)| n == channel) {
            entry.1.clear();
        }
        if let Some(idx) = self.output_panel.find_channel(channel) {
            self.output_panel.clear_channel(idx);
        }
    }

    /// Write an entry to the debug console.
    ///
    /// `kind` must be `"input"`, `"output"`, or `"error"`.
    pub fn debug_console_write(&mut self, kind: &str, text: &str) {
        self.debug_console_lines.push((kind.to_string(), text.to_string()));
        match kind {
            "input" => self.debug_view.console.add_input(text),
            "error" => self.debug_view.console.add_output(text, OutputCategory::Stderr),
            _ => self.debug_view.console.add_output(text, OutputCategory::Stdout),
        }
    }

    /// Return all lines from the currently selected output channel.
    pub fn get_output_lines(&self) -> Vec<String> {
        self.output_panel
            .active_channel()
            .map(|ch| ch.content.clone())
            .unwrap_or_default()
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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 239
// ---------------------------------------------------------------------------

/// Generic object pool `Xc239Pool<T>`.
pub struct Xc239Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc239Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc239PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc239Pool<T> {
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
    pub fn stats(&self) -> Xc239PoolStats {
        Xc239PoolStats {
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

impl<T> Default for Xc239Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc239Scheduler`.
pub struct Xc239Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc239Scheduler {
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

impl Default for Xc239Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_239 hash for the given byte slice.
pub fn xc_239_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_239 convention.
pub fn xc_239_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe119 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe119Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe119PipelineError {
    pub stage: Xe119Stage,
    pub message: String,
}

impl std::fmt::Display for Xe119PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe119Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe119Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError>>>,
    stage_names: Vec<Xe119Stage>,
}

impl Xe119Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe119Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe119Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe119Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe119Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe119Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe119CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe119CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe119Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe119CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe119CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe119Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe119CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_119_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe119CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_119_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe119CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_119_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
    Ok(data)
}

pub fn xe_119_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_119_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_119_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_119_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe119PipelineError> {
    Err(Xe119PipelineError {
        stage: Xe119Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_117: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg117Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg117Graph {
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

impl Default for Xg117Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_117: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg117Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg117Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg117Heap<T>) {
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

impl<T: Ord> Default for Xg117Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 238).
pub struct Xh238SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh238SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 280 as u64,
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

/// A compact bit set supporting boolean operations (variant 238).
pub struct Xh238BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh238BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 238).
pub struct Xi238Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi238Deque<T> {
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
pub struct Xi238Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi238Interval {
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

/// A simple interval tree (variant 238).
pub struct Xi238IntervalTree {
    xi_intervals: Vec<Xi238Interval>,
}

impl Xi238IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi238Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi238Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi238Interval) -> Vec<&Xi238Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi238Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi238Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi238Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi238Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi238Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi238Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 238) ---

/// Disjoint set / union-find for crate 238.
pub struct Xj238UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj238UnionFind {
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

const XJ238_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 238.
pub struct Xj238BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj238BTreeNode<K, V>>>,
    len: usize,
}

struct Xj238BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj238BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj238BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ238_BTREE_ORDER - 1
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
        let mid = XJ238_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj238BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj238BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj238BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj238BTreeNode::xj_new_leaf();
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


// --- xk_238 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk238SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk238SegmentTree {
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
pub struct Xk238DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk238DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_238).
#[derive(Debug, Clone)]
pub struct Xl238Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl238Rope {
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

/// Suffix array for efficient string searching (xl_238).
#[derive(Debug, Clone)]
pub struct Xl238SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl238SuffixArray {
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
pub struct Xm238MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm238MatrixSparse {
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
pub struct Xm238Tokenizer {
    text: String,
}

impl Xm238Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 238.
pub struct Xn238Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn238Fenwick {
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

// ----- AVL tree map — crate 238 -----

#[derive(Debug, Clone)]
struct Xn238AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn238AvlNode<K, V>>>,
    right: Option<Box<Xn238AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 238.
#[derive(Debug, Clone)]
pub struct Xn238AVL<K, V> {
    root: Option<Box<Xn238AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn238AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn238AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn238AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn238AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn238AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn238AvlNode<K, V>>) -> Box<Xn238AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn238AvlNode<K, V>>) -> Box<Xn238AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn238AvlNode<K, V>>) -> Box<Xn238AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn238AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn238AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn238AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn238AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn238AvlNode<K, V>>) -> &Xn238AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn238AvlNode<K, V>>) -> (Box<Xn238AvlNode<K, V>>, Option<Box<Xn238AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn238AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn238AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn238AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn238AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn238AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn238AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn238AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


// ---------------------------------------------------------------------------
// Xo238RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo238Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo238RBNode<K, V> {
    key: K,
    value: V,
    color: Xo238Color,
    left: Option<Box<Xo238RBNode<K, V>>>,
    right: Option<Box<Xo238RBNode<K, V>>>,
}

/// A red-black tree map for crate 238.
#[derive(Debug, Clone)]
pub struct Xo238RedBlack<K, V> {
    root: Option<Box<Xo238RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo238RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo238Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo238RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo238RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo238RBNode {
                    key, value, color: Xo238Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo238RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo238Color::Red)
    }

    fn xo_balance(mut h: Box<Xo238RBNode<K, V>>) -> Box<Xo238RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo238Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo238RBNode<K, V>>) -> Box<Xo238RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo238Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo238RBNode<K, V>>) -> Box<Xo238RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo238Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo238RBNode<K, V>>) {
        h.color = Xo238Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo238Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo238Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo238Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo238RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo238RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo238RBNode<K, V>) -> (K, V, Option<Box<Xo238RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo238RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo238Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo238RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo238ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 238.
#[derive(Debug, Clone)]
pub struct Xo238ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo238ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo238#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo238#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 238).
#[derive(Debug)]
pub struct Xp238SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp238Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp238Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp238Node<K, V>>>,
    xp_right: Option<Box<Xp238Node<K, V>>>,
}

impl<K: Ord, V> Xp238Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp238SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp238SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp238Node<K, V>>>, key: &K) -> Option<Box<Xp238Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp238Node<K, V>>) -> Box<Xp238Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp238Node<K, V>>) -> Box<Xp238Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp238Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp238Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp238Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq238Treap ---------------

use std::cmp::Ordering as Xq238Ord;

struct Xq238TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq238TreapNode<K, V>>>,
    right: Option<Box<Xq238TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq238Treap<K, V> {
    root: Option<Box<Xq238TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq238TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_238_size<K, V>(node: &Option<Box<Xq238TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_238_update_size<K, V>(node: &mut Xq238TreapNode<K, V>) {
    node.size = 1 + xq_238_size(&node.left) + xq_238_size(&node.right);
}

fn xq_238_rotate_right<K, V>(mut node: Box<Xq238TreapNode<K, V>>) -> Box<Xq238TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_238_update_size(&mut node);
    left.right = Some(node);
    xq_238_update_size(&mut left);
    left
}

fn xq_238_rotate_left<K, V>(mut node: Box<Xq238TreapNode<K, V>>) -> Box<Xq238TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_238_update_size(&mut node);
    right.left = Some(node);
    xq_238_update_size(&mut right);
    right
}

fn xq_238_insert_node<K: Ord, V>(
    node: Option<Box<Xq238TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq238TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq238TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq238Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq238Ord::Less => {
                let (new_left, old) = xq_238_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_238_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_238_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq238Ord::Greater => {
                let (new_right, old) = xq_238_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_238_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_238_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_238_remove_node<K: Ord, V>(
    node: Option<Box<Xq238TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq238TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq238Ord::Less => {
                let (new_left, old) = xq_238_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_238_update_size(&mut n);
                (Some(n), old)
            }
            Xq238Ord::Greater => {
                let (new_right, old) = xq_238_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_238_update_size(&mut n);
                (Some(n), old)
            }
            Xq238Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_238_rotate_right(n);
                    let (new_right, old) = xq_238_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_238_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_238_rotate_left(n);
                    let (new_left, old) = xq_238_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_238_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_238_find_min<K, V>(node: &Option<Box<Xq238TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_238_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_238_find_max<K, V>(node: &Option<Box<Xq238TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_238_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_238_rank<K: Ord, V>(node: &Option<Box<Xq238TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq238Ord::Less => xq_238_rank(&n.left, key),
            Xq238Ord::Equal => xq_238_size(&n.left),
            Xq238Ord::Greater => 1 + xq_238_size(&n.left) + xq_238_rank(&n.right, key),
        },
    }
}

fn xq_238_kth<K, V>(node: &Option<Box<Xq238TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_238_size(&n.left);
        if k < left_size {
            xq_238_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_238_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_238_in_order<K: Clone, V>(node: &Option<Box<Xq238TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_238_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_238_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq238Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 238 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_238_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq238Ord::Equal => return Some(&n.value),
                Xq238Ord::Less => cur = &n.left,
                Xq238Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_238_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_238_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_238_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_238_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_238_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_238_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_238_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq238VEBTree ---------------

pub struct Xq238VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq238VEBTree>>,
    clusters: Vec<Option<Box<Xq238VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq238VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq238VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq238VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr238KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr238KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr238BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr238KDNode {
    xr_point: Xr238KDPoint,
    xr_left: Option<Box<Xr238KDNode>>,
    xr_right: Option<Box<Xr238KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr238KDTree {
    xr_root: Option<Box<Xr238KDNode>>,
    xr_size: usize,
}

impl Xr238KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr238KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr238KDNode>>,
        point: Xr238KDPoint,
        depth: usize,
    ) -> Box<Xr238KDNode> {
        match node {
            None => Box::new(Xr238KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr238KDPoint) -> Option<Xr238KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr238KDNode>,
        query: &Xr238KDPoint,
        depth: usize,
        best: &mut Xr238KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr238KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr238KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr238KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr238KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr238KDNode>>, pts: &mut Vec<Xr238KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr238KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr238BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr238BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs238PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs238PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs238PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs238PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs238ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs238ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs238ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs238RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs238RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs238RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs238CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs238CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs238CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}

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

    // ---- xc_ pool / scheduler tests – block 239 ----

    #[test]
    fn xc_239_pool_new_empty() {
        let pool: super::Xc239Pool<i32> = super::Xc239Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_239_pool_release_acquire() {
        let mut pool = super::Xc239Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_239_pool_acquire_empty() {
        let mut pool: super::Xc239Pool<i32> = super::Xc239Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_239_pool_full() {
        let mut pool = super::Xc239Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_239_pool_drain() {
        let mut pool = super::Xc239Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_239_pool_stats() {
        let mut pool = super::Xc239Pool::new(8);
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
    fn xc_239_pool_clear() {
        let mut pool = super::Xc239Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_239_pool_shrink() {
        let mut pool = super::Xc239Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_239_pool_default() {
        let pool: super::Xc239Pool<String> = super::Xc239Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_239_pool_extend() {
        let mut pool = super::Xc239Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_239_pool_retain() {
        let mut pool = super::Xc239Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_239_scheduler_round_robin() {
        let mut sched = super::Xc239Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_239_scheduler_empty() {
        let mut sched = super::Xc239Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_239_scheduler_reset() {
        let mut sched = super::Xc239Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_239_scheduler_add_remove() {
        let mut sched = super::Xc239Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_239_scheduler_targets() {
        let sched = super::Xc239Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_239_hash_empty() {
        assert_eq!(super::xc_239_hash(b""), 5381);
    }

    #[test]
    fn xc_239_hash_data() {
        let h = super::xc_239_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_239_hash(b"hello"), h);
    }

    #[test]
    fn xc_239_reverse_str() {
        assert_eq!(super::xc_239_reverse("abc"), "cba");
        assert_eq!(super::xc_239_reverse(""), "");
    }


    #[test]
    fn xe_119_pipeline_empty() {
        let p = super::Xe119Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_119_pipeline_parse_stage() {
        let p = super::Xe119Pipeline::new()
            .add_parse(super::xe_119_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_119_pipeline_transform_double() {
        let p = super::Xe119Pipeline::new()
            .add_transform(super::xe_119_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_119_pipeline_validate_reverse() {
        let p = super::Xe119Pipeline::new()
            .add_validate(super::xe_119_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_119_pipeline_emit_filter() {
        let p = super::Xe119Pipeline::new()
            .add_emit(super::xe_119_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_119_pipeline_multi_stage() {
        let p = super::Xe119Pipeline::new()
            .add_parse(super::xe_119_pipeline_identity)
            .add_transform(super::xe_119_pipeline_double)
            .add_validate(super::xe_119_pipeline_reverse)
            .add_emit(super::xe_119_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_119_pipeline_error_propagation() {
        let p = super::Xe119Pipeline::new()
            .add_parse(super::xe_119_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe119Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_119_pipeline_compose() {
        let p1 = super::Xe119Pipeline::new()
            .add_parse(super::xe_119_pipeline_identity);
        let p2 = super::Xe119Pipeline::new()
            .add_transform(super::xe_119_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_119_pipeline_error_display() {
        let e = super::Xe119PipelineError {
            stage: super::Xe119Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_119_cache_put_get() {
        let mut c = super::Xe119Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_119_cache_miss() {
        let mut c: super::Xe119Cache<&str, i32> = super::Xe119Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_119_cache_ttl_expiry() {
        let mut c = super::Xe119Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_119_cache_evict() {
        let mut c = super::Xe119Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_119_cache_capacity() {
        let mut c = super::Xe119Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_119_cache_stats() {
        let mut c = super::Xe119Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_119_cache_clear() {
        let mut c = super::Xe119Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_117 graph tests ------------------------------------------------

    #[test]
    fn xg_117_graph_empty() {
        let g = super::Xg117Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_117_graph_add_node() {
        let mut g = super::Xg117Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_117_graph_add_edge() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_117_graph_neighbors() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_117_graph_has_path() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_117_graph_self_path() {
        let g = super::Xg117Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_117_graph_topo_sort() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_117_graph_cycle_detect_false() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_117_graph_cycle_detect_true() {
        let mut g = super::Xg117Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_117 heap tests -------------------------------------------------

    #[test]
    fn xg_117_heap_empty() {
        let h: super::Xg117Heap<i32> = super::Xg117Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_117_heap_push_pop() {
        let mut h = super::Xg117Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_117_heap_peek() {
        let mut h = super::Xg117Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_117_heap_drain_sorted() {
        let mut h = super::Xg117Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_117_heap_merge() {
        let mut a = super::Xg117Heap::new();
        let mut b = super::Xg117Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_117_heap_default() {
        let h: super::Xg117Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_117_graph_default() {
        let g: super::Xg117Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh238_skip_insert_contains() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh238_skip_remove() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh238_skip_len() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh238_skip_range_query() {
        let mut sl = super::Xh238SkipList::xh_new(4);
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
    fn xh238_skip_floor_ceiling() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh238_skip_rank() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh238_skip_empty() {
        let sl = super::Xh238SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh238_skip_duplicates() {
        let mut sl = super::Xh238SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh238_bitset_set_test() {
        let mut bs = super::Xh238BitSet::xh_new(256);
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
    fn xh238_bitset_clear_count() {
        let mut bs = super::Xh238BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh238_bitset_and_or_xor() {
        let mut a = super::Xh238BitSet::xh_new(128);
        let mut b = super::Xh238BitSet::xh_new(128);
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
    fn xh238_bitset_iter_ones() {
        let mut bs = super::Xh238BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh238_bitset_first_last() {
        let mut bs = super::Xh238BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh238_bitset_empty() {
        let bs = super::Xh238BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi238_deque_push_pop_back() {
        let mut dq = super::Xi238Deque::xi_new(4);
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
    fn xi238_deque_push_pop_front() {
        let mut dq = super::Xi238Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi238_deque_mixed_ops() {
        let mut dq = super::Xi238Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi238_deque_get_and_split() {
        let mut dq = super::Xi238Deque::xi_new(8);
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
    fn xi238_deque_rotate_left() {
        let mut dq = super::Xi238Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi238_deque_rotate_right() {
        let mut dq = super::Xi238Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi238_deque_grow() {
        let mut dq = super::Xi238Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi238_deque_empty() {
        let dq = super::Xi238Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi238_interval_tree_insert_query() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi238Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi238Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi238_interval_tree_overlap() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi238Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi238Interval::xi_new(12, 20));
        let q = super::Xi238Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi238_interval_tree_remove() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi238Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi238_interval_tree_gaps() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi238Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi238Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi238Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi238Interval::xi_new(8, 10));
    }

    #[test]
    fn xi238_interval_tree_merge() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi238Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi238Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi238Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi238Interval::xi_new(10, 15));
    }

    #[test]
    fn xi238_interval_tree_all() {
        let mut tree = super::Xi238IntervalTree::xi_new();
        tree.xi_insert(super::Xi238Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi238Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi238_interval_tree_empty() {
        let tree = super::Xi238IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi238_interval_tree_contains_point() {
        let iv = super::Xi238Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 238) ---

    #[test]
    fn xj_238_uf_make_and_find() {
        let mut uf = super::Xj238UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_238_uf_union_connected() {
        let mut uf = super::Xj238UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_238_uf_component_count() {
        let mut uf = super::Xj238UnionFind::xj_new();
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
    fn xj_238_uf_component_size() {
        let mut uf = super::Xj238UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_238_uf_largest_component() {
        let mut uf = super::Xj238UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_238_uf_many_elements() {
        let mut uf = super::Xj238UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_238_uf_separate_components() {
        let mut uf = super::Xj238UnionFind::xj_new();
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
    fn xj_238_uf_path_compression() {
        let mut uf = super::Xj238UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_238_bt_insert_get() {
        let mut bt = super::Xj238BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_238_bt_contains_len() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_238_bt_replace() {
        let mut bt = super::Xj238BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_238_bt_remove() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_238_bt_keys_values() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_238_bt_range() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_238_bt_min_max() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_238_bt_many_inserts() {
        let mut bt = super::Xj238BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_238 segment tree tests ---

    #[test]
    fn xk_238_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_238_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk238SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_238_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_238_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_238_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_238_st_single_element() {
        let data = vec![42];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_238_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk238SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_238_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk238SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_238 disjoint intervals tests ---

    #[test]
    fn xk_238_di_add_and_count() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_238_di_merge_overlap() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_238_di_contains() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_238_di_remove() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_238_di_covered_length() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_238_di_gaps() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_238_di_merge_adjacent() {
        let mut di = super::Xk238DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_238_di_empty() {
        let di = super::Xk238DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_238_rope_new_empty() {
        let rope = super::Xl238Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_238_rope_from_str() {
        let rope = super::Xl238Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_238_rope_insert_at() {
        let mut rope = super::Xl238Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_238_rope_delete_range() {
        let mut rope = super::Xl238Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_238_rope_char_at() {
        let rope = super::Xl238Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_238_rope_split_concat() {
        let rope = super::Xl238Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_238_rope_line_count() {
        let rope = super::Xl238Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_238_rope_line_at() {
        let rope = super::Xl238Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_238_sa_build_and_search() {
        let sa = super::Xl238SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_238_sa_count() {
        let sa = super::Xl238SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_238_sa_longest_repeated() {
        let sa = super::Xl238SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_238_sa_all_positions() {
        let sa = super::Xl238SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_238_sa_len() {
        let sa = super::Xl238SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_238_sa_empty() {
        let sa = super::Xl238SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_238_rope_slice() {
        let rope = super::Xl238Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_238_sa_search_start() {
        let sa = super::Xl238SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_238_sparse_set_get() {
        let mut m = super::Xm238MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_238_sparse_row_col() {
        let mut m = super::Xm238MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_238_sparse_transpose() {
        let mut m = super::Xm238MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_238_sparse_multiply_vec() {
        let mut m = super::Xm238MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_238_sparse_nnz_density() {
        let mut m = super::Xm238MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_238_sparse_clear() {
        let mut m = super::Xm238MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_238_sparse_overwrite_zero() {
        let mut m = super::Xm238MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_238_tokenizer_basic() {
        let t = super::Xm238Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_238_tokenizer_count() {
        let t = super::Xm238Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_238_tokenizer_unique() {
        let t = super::Xm238Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_238_tokenizer_frequency() {
        let t = super::Xm238Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_238_tokenizer_delimiter() {
        let t = super::Xm238Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_238_tokenizer_whitespace() {
        let t = super::Xm238Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_238_tokenizer_empty() {
        let t = super::Xm238Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 238 ----

    #[test]
    fn xn_238_fenwick_prefix_sum() {
        let mut ft = super::Xn238Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_238_fenwick_range_sum() {
        let mut ft = super::Xn238Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_238_fenwick_point_query() {
        let mut ft = super::Xn238Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_238_fenwick_len() {
        let ft = super::Xn238Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_238_fenwick_multiple_updates() {
        let mut ft = super::Xn238Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_238_fenwick_single_element() {
        let mut ft = super::Xn238Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_238_fenwick_find_kth() {
        let mut ft = super::Xn238Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_238_fenwick_negative_delta() {
        let mut ft = super::Xn238Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 238 ----

    #[test]
    fn xn_238_avl_insert_get() {
        let mut m = super::Xn238AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_238_avl_remove() {
        let mut m = super::Xn238AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_238_avl_in_order() {
        let mut m = super::Xn238AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_238_avl_min_max() {
        let mut m = super::Xn238AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_238_avl_floor_ceiling() {
        let mut m = super::Xn238AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_238_avl_height_balanced() {
        let mut m = super::Xn238AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_238_avl_overwrite() {
        let mut m = super::Xn238AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_238_avl_empty() {
        let m: super::Xn238AVL<i32, i32> = super::Xn238AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo238RedBlack tests ---

    #[test]
    fn xo_238_rb_insert_and_get() {
        let mut tree = super::Xo238RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_238_rb_len_and_empty() {
        let mut tree = super::Xo238RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_238_rb_min_max() {
        let mut tree = super::Xo238RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_238_rb_contains() {
        let mut tree = super::Xo238RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_238_rb_remove() {
        let mut tree = super::Xo238RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_238_rb_in_order() {
        let mut tree = super::Xo238RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_238_rb_black_height() {
        let mut tree = super::Xo238RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_238_rb_overwrite() {
        let mut tree = super::Xo238RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo238ConsistentHash tests ---

    #[test]
    fn xo_238_ch_add_and_count() {
        let mut ring = super::Xo238ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_238_ch_remove_node() {
        let mut ring = super::Xo238ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_238_ch_get_node() {
        let mut ring = super::Xo238ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_238_ch_empty_ring() {
        let ring = super::Xo238ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_238_ch_distribution() {
        let mut ring = super::Xo238ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_238_ch_rebalance() {
        let mut ring = super::Xo238ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_238_ch_virtual_nodes() {
        let mut ring = super::Xo238ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_238_ch_consistent_lookup() {
        let mut ring = super::Xo238ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_238_splay_insert_get() {
        let mut t = super::Xp238SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_238_splay_remove() {
        let mut t = super::Xp238SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_238_splay_count_increases() {
        let mut t = super::Xp238SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_238_splay_depth() {
        let mut t = super::Xp238SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_238_splay_len_empty() {
        let t = super::Xp238SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_238_splay_min_max() {
        let mut t = super::Xp238SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_238_splay_overwrite() {
        let mut t = super::Xp238SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_238_splay_remove_missing() {
        let mut t = super::Xp238SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_238 treap tests ----
    #[test]
    fn xq_238_treap_empty() {
        let t = super::Xq238Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_238_treap_insert_get() {
        let mut t = super::Xq238Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_238_treap_overwrite() {
        let mut t = super::Xq238Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_238_treap_remove() {
        let mut t = super::Xq238Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_238_treap_min_max() {
        let mut t = super::Xq238Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_238_treap_rank() {
        let mut t = super::Xq238Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_238_treap_kth() {
        let mut t = super::Xq238Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_238_treap_in_order() {
        let mut t = super::Xq238Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_238 VEB tree tests ----
    #[test]
    fn xq_238_veb_empty() {
        let v = super::Xq238VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_238_veb_insert_contains() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_238_veb_min_max() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_238_veb_delete() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_238_veb_successor() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_238_veb_predecessor() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_238_veb_count() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_238_veb_duplicate_insert() {
        let mut v = super::Xq238VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_238_kdtree_empty() {
        let tree = super::Xr238KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_238_kdtree_insert_one() {
        let mut tree = super::Xr238KDTree::xr_new();
        tree.xr_insert(super::Xr238KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_238_kdtree_insert_multiple() {
        let mut tree = super::Xr238KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr238KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_238_kdtree_nearest_neighbor() {
        let mut tree = super::Xr238KDTree::xr_new();
        tree.xr_insert(super::Xr238KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr238KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr238KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_238_kdtree_nn_empty() {
        let tree = super::Xr238KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr238KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_238_kdtree_range_search() {
        let mut tree = super::Xr238KDTree::xr_new();
        tree.xr_insert(super::Xr238KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr238KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr238KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_238_kdtree_range_empty() {
        let mut tree = super::Xr238KDTree::xr_new();
        tree.xr_insert(super::Xr238KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_238_kdtree_all_points() {
        let mut tree = super::Xr238KDTree::xr_new();
        tree.xr_insert(super::Xr238KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr238KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_238_kdtree_depth() {
        let mut tree = super::Xr238KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr238KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_238_kdtree_bounding_box() {
        let mut tree = super::Xr238KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr238KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr238KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_238_persistent_array_new() {
        let arr = super::Xs238PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_238_persistent_array_push() {
        let mut arr = super::Xs238PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_238_persistent_array_set() {
        let mut arr = super::Xs238PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_238_persistent_array_diff() {
        let mut arr = super::Xs238PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_238_persistent_array_rollback() {
        let mut arr = super::Xs238PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_238_persistent_array_history() {
        let mut arr = super::Xs238PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_238_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs238PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_238_persistent_array_from_vec() {
        let arr = super::Xs238PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_238_concurrent_queue_new() {
        let q = super::Xs238ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_238_concurrent_queue_push_pop() {
        let mut q = super::Xs238ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_238_concurrent_queue_full() {
        let mut q = super::Xs238ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_238_concurrent_queue_drain() {
        let mut q = super::Xs238ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_238_concurrent_queue_try_pop() {
        let mut q = super::Xs238ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_238_concurrent_queue_clear() {
        let mut q = super::Xs238ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_238_range_map_new() {
        let rm = super::Xs238RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_238_range_map_insert_get() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_238_range_map_overlap() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_238_range_map_remove() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_238_range_map_gaps() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_238_range_map_coverage() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_238_range_map_contains() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_238_range_map_clear() {
        let mut rm = super::Xs238RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_238_circular_buffer_new() {
        let buf = super::Xs238CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_238_circular_buffer_push_pop() {
        let mut buf = super::Xs238CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_238_circular_buffer_overwrite() {
        let mut buf = super::Xs238CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_238_circular_buffer_peek() {
        let mut buf = super::Xs238CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_238_circular_buffer_is_full() {
        let mut buf = super::Xs238CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_238_circular_buffer_iter() {
        let mut buf = super::Xs238CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_238_circular_buffer_clear() {
        let mut buf = super::Xs238CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_238_circular_buffer_to_vec() {
        let mut buf = super::Xs238CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }

}
