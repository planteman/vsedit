//! Extensions marketplace view.
//!
//! RPC bridge between the extension host and the main thread for custom views.

use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_view";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewMessage {
    CreateWebviewPanel {
        view_type: String,
        title: String,
        column: ViewColumn,
    },
    DisposePanel {
        panel_id: String,
    },
    RevealPanel {
        panel_id: String,
        column: ViewColumn,
        preserve_focus: bool,
    },
    SetTitle {
        panel_id: String,
        title: String,
    },
    SetHtml {
        panel_id: String,
        html: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ViewColumn {
    Active,
    Beside,
    One,
    Two,
    Three,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebviewPanel {
    pub id: String,
    pub view_type: String,
    pub title: String,
    pub column: ViewColumn,
    pub html: String,
    pub is_visible: bool,
}

// ── Bridge ──

pub struct ViewBridge {
    panels: Vec<WebviewPanel>,
    next_id: u64,
}

impl ViewBridge {
    pub fn new() -> Self {
        Self {
            panels: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_panel(&mut self, view_type: &str, title: &str, column: ViewColumn) -> String {
        let id = format!("panel-{}", self.next_id);
        self.next_id += 1;
        self.panels.push(WebviewPanel {
            id: id.clone(),
            view_type: view_type.to_string(),
            title: title.to_string(),
            column,
            html: String::new(),
            is_visible: true,
        });
        id
    }

    pub fn dispose_panel(&mut self, panel_id: &str) -> bool {
        let before = self.panels.len();
        self.panels.retain(|p| p.id != panel_id);
        self.panels.len() < before
    }

    pub fn get_panel(&self, id: &str) -> Option<&WebviewPanel> {
        self.panels.iter().find(|p| p.id == id)
    }

    pub fn handle_message(&mut self, msg: &ViewMessage) -> serde_json::Value {
        match msg {
            ViewMessage::CreateWebviewPanel {
                view_type,
                title,
                column,
            } => {
                let id = self.create_panel(view_type, title, *column);
                serde_json::json!({"panelId": id})
            }
            ViewMessage::DisposePanel { panel_id } => {
                let ok = self.dispose_panel(panel_id);
                serde_json::json!({"disposed": ok})
            }
            ViewMessage::RevealPanel {
                panel_id,
                column,
                preserve_focus,
            } => {
                if let Some(p) = self.panels.iter_mut().find(|p| p.id == *panel_id) {
                    p.column = *column;
                    p.is_visible = true;
                    serde_json::json!({"revealed": true, "preserveFocus": preserve_focus})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            ViewMessage::SetTitle { panel_id, title } => {
                if let Some(p) = self.panels.iter_mut().find(|p| p.id == *panel_id) {
                    p.title = title.clone();
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            ViewMessage::SetHtml { panel_id, html } => {
                if let Some(p) = self.panels.iter_mut().find(|p| p.id == *panel_id) {
                    p.html = html.clone();
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
        }
    }
}

impl Default for ViewBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Types ──

/// Errors that can occur during view operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    /// The referenced panel does not exist.
    PanelNotFound(String),
    /// A validation rule was violated.
    InvalidInput(String),
    /// The panel limit has been reached.
    PanelLimitExceeded { limit: usize },
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewError::PanelNotFound(id) => write!(f, "panel not found: {id}"),
            ViewError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            ViewError::PanelLimitExceeded { limit } => {
                write!(f, "panel limit exceeded (max {limit})")
            }
        }
    }
}

impl std::error::Error for ViewError {}

impl ViewError {
    /// Returns `true` if this is a `PanelNotFound` error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, ViewError::PanelNotFound(_))
    }

    /// Returns `true` if this is an `InvalidInput` error.
    pub fn is_invalid_input(&self) -> bool {
        matches!(self, ViewError::InvalidInput(_))
    }

    /// Returns `true` if this is a `PanelLimitExceeded` error.
    pub fn is_limit_exceeded(&self) -> bool {
        matches!(self, ViewError::PanelLimitExceeded { .. })
    }

    /// If this is a `PanelNotFound` error, returns the missing panel id.
    pub fn panel_id(&self) -> Option<&str> {
        match self {
            ViewError::PanelNotFound(id) => Some(id),
            _ => None,
        }
    }
}

// ── Display impls ──

impl std::fmt::Display for ViewColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewColumn::Active => write!(f, "Active"),
            ViewColumn::Beside => write!(f, "Beside"),
            ViewColumn::One => write!(f, "1"),
            ViewColumn::Two => write!(f, "2"),
            ViewColumn::Three => write!(f, "3"),
        }
    }
}

impl std::fmt::Display for WebviewPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} (type={}, col={}, visible={})",
            self.id, self.title, self.view_type, self.column, self.is_visible,
        )
    }
}

// ── WebviewPanel helpers ──

impl WebviewPanel {
    /// Returns the byte length of the HTML content.
    pub fn html_byte_len(&self) -> usize {
        self.html.len()
    }

    /// Returns `true` if the panel has non-empty HTML content.
    pub fn has_content(&self) -> bool {
        !self.html.is_empty()
    }

    /// Clears the HTML content of the panel.
    pub fn clear_html(&mut self) {
        self.html.clear();
    }

    /// Renames the panel, returning the old title.
    pub fn rename(&mut self, new_title: impl Into<String>) -> String {
        std::mem::replace(&mut self.title, new_title.into())
    }

    /// Returns `true` if the panel's title or view_type contains `query`
    /// (case-insensitive).
    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q) || self.view_type.to_lowercase().contains(&q)
    }

    /// Returns `true` if this panel has the given `view_type`.
    pub fn is_type(&self, vt: &str) -> bool {
        self.view_type == vt
    }

    /// Sets visibility and returns the previous value.
    pub fn set_visibility(&mut self, visible: bool) -> bool {
        std::mem::replace(&mut self.is_visible, visible)
    }

    /// Returns a short one-line summary: `"id: title (type)"`.
    pub fn summary(&self) -> String {
        format!("{}: {} ({})", self.id, self.title, self.view_type)
    }

    /// Wraps the current HTML in a basic document skeleton if it does not
    /// already begin with `<!DOCTYPE` or `<html`.
    pub fn wrap_in_document(&mut self) {
        let trimmed = self.html.trim_start();
        if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<html") {
            return;
        }
        self.html = format!(
            "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"></head><body>\n{}\n</body></html>",
            self.html
        );
    }
}

// ── ViewColumn helpers ──

impl ViewColumn {
    /// Converts a 1-based column number to a `ViewColumn`, returning `None`
    /// for out-of-range values.
    pub fn from_number(n: u8) -> Option<Self> {
        match n {
            1 => Some(ViewColumn::One),
            2 => Some(ViewColumn::Two),
            3 => Some(ViewColumn::Three),
            _ => None,
        }
    }

    /// Returns the 1-based column index, or `None` for symbolic columns.
    pub fn to_number(self) -> Option<u8> {
        match self {
            ViewColumn::One => Some(1),
            ViewColumn::Two => Some(2),
            ViewColumn::Three => Some(3),
            ViewColumn::Active | ViewColumn::Beside => None,
        }
    }

    /// Returns `true` for the symbolic (non-numeric) columns.
    pub fn is_symbolic(self) -> bool {
        matches!(self, ViewColumn::Active | ViewColumn::Beside)
    }

    /// Returns `true` for the numeric (non-symbolic) columns.
    pub fn is_numeric(self) -> bool {
        !self.is_symbolic()
    }

    /// Returns all numeric column variants.
    pub fn all_numeric() -> [ViewColumn; 3] {
        [ViewColumn::One, ViewColumn::Two, ViewColumn::Three]
    }

    /// Advances to the next numeric column, wrapping from Three back to One.
    /// Symbolic columns resolve to `One`.
    pub fn next_column(self) -> ViewColumn {
        match self {
            ViewColumn::One => ViewColumn::Two,
            ViewColumn::Two => ViewColumn::Three,
            ViewColumn::Three => ViewColumn::One,
            ViewColumn::Active | ViewColumn::Beside => ViewColumn::One,
        }
    }

    /// Moves to the previous numeric column, wrapping from One to Three.
    /// Symbolic columns resolve to `One`.
    pub fn prev_column(self) -> ViewColumn {
        match self {
            ViewColumn::One => ViewColumn::Three,
            ViewColumn::Two => ViewColumn::One,
            ViewColumn::Three => ViewColumn::Two,
            ViewColumn::Active | ViewColumn::Beside => ViewColumn::One,
        }
    }
}

// ── PanelBuilder ──

/// Builder for constructing a `WebviewPanel` with validation.
#[derive(Debug, Clone)]
pub struct PanelBuilder {
    view_type: Option<String>,
    title: Option<String>,
    column: ViewColumn,
    html: String,
    visible: bool,
}

impl PanelBuilder {
    pub fn new() -> Self {
        Self {
            view_type: None,
            title: None,
            column: ViewColumn::Active,
            html: String::new(),
            visible: true,
        }
    }

    pub fn view_type(mut self, vt: impl Into<String>) -> Self {
        self.view_type = Some(vt.into());
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn column(mut self, c: ViewColumn) -> Self {
        self.column = c;
        self
    }

    pub fn html(mut self, h: impl Into<String>) -> Self {
        self.html = h.into();
        self
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }

    /// Validates and builds the panel, returning a `ViewMessage::CreateWebviewPanel`
    /// that can be sent through the bridge.  Returns an error if required fields
    /// are missing or contain only whitespace.
    pub fn build(self) -> Result<(ViewMessage, String), ViewError> {
        let view_type = self
            .view_type
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ViewError::InvalidInput("view_type is required".into()))?;
        let title = self
            .title
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ViewError::InvalidInput("title is required".into()))?;
        let msg = ViewMessage::CreateWebviewPanel {
            view_type,
            title,
            column: self.column,
        };
        Ok((msg, self.html))
    }
}

impl Default for PanelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Extended ViewBridge methods ──

/// Maximum number of panels a single bridge may hold.
pub const MAX_PANELS: usize = 64;

impl ViewBridge {
    /// Creates a panel through the builder result, returning the new panel id.
    pub fn create_from_builder(
        &mut self,
        builder: PanelBuilder,
    ) -> Result<String, ViewError> {
        if self.panels.len() >= MAX_PANELS {
            return Err(ViewError::PanelLimitExceeded { limit: MAX_PANELS });
        }
        let (msg, html) = builder.build()?;
        let result = self.handle_message(&msg);
        let panel_id = result["panelId"]
            .as_str()
            .expect("handle_message always returns panelId for CreateWebviewPanel")
            .to_string();
        if !html.is_empty() {
            self.handle_message(&ViewMessage::SetHtml {
                panel_id: panel_id.clone(),
                html,
            });
        }
        Ok(panel_id)
    }

    /// Returns an iterator over all currently managed panels.
    pub fn panels(&self) -> impl Iterator<Item = &WebviewPanel> {
        self.panels.iter()
    }

    /// Returns the total number of tracked panels.
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    /// Returns a mutable reference to a panel, or a `ViewError` if not found.
    pub fn get_panel_mut(&mut self, id: &str) -> Result<&mut WebviewPanel, ViewError> {
        self.panels
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| ViewError::PanelNotFound(id.to_string()))
    }

    /// Finds all panels whose title contains `query` (case-insensitive).
    pub fn search_panels(&self, query: &str) -> Vec<&WebviewPanel> {
        let lower = query.to_lowercase();
        self.panels
            .iter()
            .filter(|p| p.title.to_lowercase().contains(&lower))
            .collect()
    }

    /// Returns panels grouped by their `ViewColumn`.
    pub fn panels_by_column(&self) -> std::collections::HashMap<String, Vec<&WebviewPanel>> {
        let mut map: std::collections::HashMap<String, Vec<&WebviewPanel>> =
            std::collections::HashMap::new();
        for panel in &self.panels {
            map.entry(panel.column.to_string())
                .or_default()
                .push(panel);
        }
        map
    }

    /// Moves a panel to a different column, returning an error if the panel is
    /// not found.
    pub fn move_panel(&mut self, id: &str, column: ViewColumn) -> Result<(), ViewError> {
        let panel = self.get_panel_mut(id)?;
        panel.column = column;
        Ok(())
    }

    /// Hides a panel without disposing it.
    pub fn hide_panel(&mut self, id: &str) -> Result<(), ViewError> {
        let panel = self.get_panel_mut(id)?;
        panel.is_visible = false;
        Ok(())
    }

    /// Returns only the visible panels.
    pub fn visible_panels(&self) -> Vec<&WebviewPanel> {
        self.panels.iter().filter(|p| p.is_visible).collect()
    }

    /// Disposes all panels, returning the number removed.
    pub fn dispose_all(&mut self) -> usize {
        let count = self.panels.len();
        self.panels.clear();
        count
    }

    /// Produces a JSON summary of bridge state suitable for diagnostics.
    pub fn diagnostics(&self) -> serde_json::Value {
        let visible = self.panels.iter().filter(|p| p.is_visible).count();
        let hidden = self.panels.len() - visible;
        serde_json::json!({
            "totalPanels": self.panels.len(),
            "visiblePanels": visible,
            "hiddenPanels": hidden,
            "nextId": self.next_id,
        })
    }
}

/// Initialize the view extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Extensions View ──

use vsedit_ext_mgmt::{GalleryExtension, InstalledExtension};

/// Active tab in the extensions view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionsTab {
    Installed,
    Recommended,
    SearchResults,
}

impl Default for ExtensionsTab {
    fn default() -> Self {
        Self::Installed
    }
}

impl std::fmt::Display for ExtensionsTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installed => write!(f, "Installed"),
            Self::Recommended => write!(f, "Recommended"),
            Self::SearchResults => write!(f, "Search Results"),
        }
    }
}

/// Rendering entry for a single extension in the list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionListItem {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub icon: Option<String>,
    pub is_enabled: bool,
    pub is_installed: bool,
    pub description: String,
}

impl ExtensionListItem {
    /// Create from an installed extension.
    pub fn from_installed(ext: &InstalledExtension) -> Self {
        Self {
            id: ext.id.clone(),
            name: ext.manifest.display_name.clone(),
            publisher: ext.manifest.publisher.clone(),
            version: ext.version.clone(),
            icon: None,
            is_enabled: ext.is_enabled,
            is_installed: true,
            description: ext.manifest.description.clone(),
        }
    }

    /// Create from a gallery search result.
    pub fn from_gallery(ext: &GalleryExtension) -> Self {
        Self {
            id: ext.id.clone(),
            name: ext.display_name.clone(),
            publisher: ext.publisher.clone(),
            version: ext.version.clone(),
            icon: None,
            is_enabled: false,
            is_installed: false,
            description: ext.description.clone(),
        }
    }

    /// Render as a single-line summary for TUI.
    pub fn render_line(&self) -> String {
        let status = if self.is_installed {
            if self.is_enabled { "✓" } else { "○" }
        } else {
            " "
        };
        format!(
            "[{status}] {} — {} v{} ({})",
            self.name, self.publisher, self.version, self.id,
        )
    }

    /// Returns the qualified name in `publisher.name` form.
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.publisher, self.name)
    }

    /// Returns `true` if `query` appears in the name, publisher, description,
    /// or id (case-insensitive).
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.publisher.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.id.to_lowercase().contains(&q)
    }

    /// Toggles the enabled flag and returns the new value.
    pub fn toggle_enabled(&mut self) -> bool {
        self.is_enabled = !self.is_enabled;
        self.is_enabled
    }

    /// Returns a human-readable status label.
    pub fn status_label(&self) -> &'static str {
        if !self.is_installed {
            "Not Installed"
        } else if self.is_enabled {
            "Enabled"
        } else {
            "Disabled"
        }
    }
}

/// Detail view data for a single extension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionDetailView {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub description: String,
    pub is_installed: bool,
    pub is_enabled: bool,
    pub dependencies: Vec<String>,
}

impl ExtensionDetailView {
    /// Create from an installed extension.
    pub fn from_installed(ext: &InstalledExtension) -> Self {
        Self {
            id: ext.id.clone(),
            name: ext.manifest.display_name.clone(),
            publisher: ext.manifest.publisher.clone(),
            version: ext.version.clone(),
            description: ext.manifest.description.clone(),
            is_installed: true,
            is_enabled: ext.is_enabled,
            dependencies: ext.manifest.extension_dependencies.clone(),
        }
    }

    /// Create from a gallery extension.
    pub fn from_gallery(ext: &GalleryExtension) -> Self {
        Self {
            id: ext.id.clone(),
            name: ext.display_name.clone(),
            publisher: ext.publisher.clone(),
            version: ext.version.clone(),
            description: ext.description.clone(),
            is_installed: false,
            is_enabled: false,
            dependencies: Vec::new(),
        }
    }

    /// Render a multi-line detail string for TUI display.
    pub fn render(&self) -> String {
        let status = if self.is_installed {
            if self.is_enabled { "Enabled" } else { "Disabled" }
        } else {
            "Not Installed"
        };
        let deps = if self.dependencies.is_empty() {
            "None".to_string()
        } else {
            self.dependencies.join(", ")
        };
        format!(
            "{}\nPublisher: {}\nVersion: {}\nStatus: {status}\nID: {}\nDependencies: {deps}\n\n{}",
            self.name, self.publisher, self.version, self.id, self.description,
        )
    }

    /// Returns `true` if this extension has any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    /// Returns the number of dependencies.
    pub fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    /// Returns a human-readable status label.
    pub fn status_label(&self) -> &'static str {
        if !self.is_installed {
            "Not Installed"
        } else if self.is_enabled {
            "Enabled"
        } else {
            "Disabled"
        }
    }

    /// Returns `true` if `query` appears in the name, publisher, description,
    /// or id (case-insensitive).
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.publisher.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.id.to_lowercase().contains(&q)
    }
}

/// State for the extensions view panel.
#[derive(Debug, Clone)]
pub struct ExtensionsViewState {
    pub active_tab: ExtensionsTab,
    pub search_query: String,
    pub installed_items: Vec<ExtensionListItem>,
    pub search_results: Vec<ExtensionListItem>,
    pub recommended_items: Vec<ExtensionListItem>,
    pub selected_index: usize,
    pub detail: Option<ExtensionDetailView>,
}

impl ExtensionsViewState {
    pub fn new() -> Self {
        Self {
            active_tab: ExtensionsTab::Installed,
            search_query: String::new(),
            installed_items: Vec::new(),
            search_results: Vec::new(),
            recommended_items: Vec::new(),
            selected_index: 0,
            detail: None,
        }
    }

    /// Get the items list for the currently active tab.
    pub fn active_items(&self) -> &[ExtensionListItem] {
        match self.active_tab {
            ExtensionsTab::Installed => &self.installed_items,
            ExtensionsTab::Recommended => &self.recommended_items,
            ExtensionsTab::SearchResults => &self.search_results,
        }
    }

    /// Switch to a different tab, resetting selection.
    pub fn switch_tab(&mut self, tab: ExtensionsTab) {
        self.active_tab = tab;
        self.selected_index = 0;
        self.detail = None;
    }

    /// Set the search query and switch to the search results tab.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.active_tab = ExtensionsTab::SearchResults;
        self.selected_index = 0;
    }

    /// Load installed extensions into the view.
    pub fn load_installed(&mut self, extensions: &[InstalledExtension]) {
        self.installed_items = extensions
            .iter()
            .map(ExtensionListItem::from_installed)
            .collect();
    }

    /// Load search results from gallery extensions.
    pub fn load_search_results(&mut self, extensions: &[GalleryExtension]) {
        self.search_results = extensions
            .iter()
            .map(ExtensionListItem::from_gallery)
            .collect();
    }

    /// Get the currently selected item.
    pub fn selected_item(&self) -> Option<&ExtensionListItem> {
        self.active_items().get(self.selected_index)
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let len = self.active_items().len();
        if len > 0 && self.selected_index < len - 1 {
            self.selected_index += 1;
        }
    }

    /// Render all lines for the current tab.
    pub fn render_list(&self) -> Vec<String> {
        self.active_items()
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let prefix = if i == self.selected_index { "▸ " } else { "  " };
                format!("{prefix}{}", item.render_line())
            })
            .collect()
    }

    /// Count items in the current tab.
    pub fn item_count(&self) -> usize {
        self.active_items().len()
    }

    /// Clears the search query and switches back to the Installed tab.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.switch_tab(ExtensionsTab::Installed);
    }

    /// Returns `true` if the search query is non-empty.
    pub fn has_search(&self) -> bool {
        !self.search_query.is_empty()
    }

    /// Count of enabled installed extensions.
    pub fn enabled_count(&self) -> usize {
        self.installed_items.iter().filter(|i| i.is_enabled).count()
    }

    /// Count of disabled installed extensions.
    pub fn disabled_count(&self) -> usize {
        self.installed_items
            .iter()
            .filter(|i| i.is_installed && !i.is_enabled)
            .count()
    }

    /// Finds an extension by its id across all lists.
    pub fn find_by_id(&self, id: &str) -> Option<&ExtensionListItem> {
        self.installed_items
            .iter()
            .chain(self.recommended_items.iter())
            .chain(self.search_results.iter())
            .find(|item| item.id == id)
    }

    /// Render tab bar.
    pub fn render_tab_bar(&self) -> String {
        let tabs = [
            ExtensionsTab::Installed,
            ExtensionsTab::Recommended,
            ExtensionsTab::SearchResults,
        ];
        tabs.iter()
            .map(|t| {
                if *t == self.active_tab {
                    format!("[{t}]")
                } else {
                    format!(" {t} ")
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

impl Default for ExtensionsViewState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Extension Browser View (ratatui) ──

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Widget};

/// Active tab in the extension browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionTab {
    Installed,
    Search,
}

/// Information about a single extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub description: String,
    pub version: String,
    pub installed: bool,
    pub enabled: bool,
}

/// A browsable extension list rendered with ratatui widgets.
pub struct ExtensionBrowserView {
    /// Currently installed extensions.
    pub installed: Vec<ExtensionInfo>,
    /// Search results from marketplace.
    pub search_results: Vec<ExtensionInfo>,
    /// Current search query.
    pub search_query: String,
    /// Selected index in the list.
    pub selected: usize,
    /// Active tab: installed vs search.
    pub active_tab: ExtensionTab,
}

impl ExtensionBrowserView {
    pub fn new() -> Self {
        Self {
            installed: Vec::new(),
            search_results: Vec::new(),
            search_query: String::new(),
            selected: 0,
            active_tab: ExtensionTab::Installed,
        }
    }

    /// The list currently displayed based on the active tab.
    pub fn active_list(&self) -> &[ExtensionInfo] {
        match self.active_tab {
            ExtensionTab::Installed => &self.installed,
            ExtensionTab::Search => &self.search_results,
        }
    }

    pub fn add_installed(&mut self, ext: ExtensionInfo) {
        self.installed.push(ext);
    }

    pub fn set_search_results(&mut self, results: Vec<ExtensionInfo>) {
        self.search_results = results;
        self.active_tab = ExtensionTab::Search;
        self.selected = 0;
    }

    pub fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_selection_down(&mut self) {
        let len = self.active_list().len();
        if len > 0 && self.selected < len - 1 {
            self.selected += 1;
        }
    }

    /// Render the extension browser view into the given area.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Length(2), // tabs
            Constraint::Length(3), // search input
            Constraint::Min(1),   // list
        ])
        .split(area);

        // Tab bar
        let tab_titles: Vec<Line<'_>> = vec![
            Line::from("INSTALLED"),
            Line::from("SEARCH"),
        ];
        let selected_tab = match self.active_tab {
            ExtensionTab::Installed => 0,
            ExtensionTab::Search => 1,
        };
        let tabs = Tabs::new(tab_titles)
            .select(selected_tab)
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        Widget::render(tabs, chunks[0], buf);

        // Search input
        let search_text = if self.search_query.is_empty() {
            "Type to search extensions...".to_string()
        } else {
            self.search_query.clone()
        };
        let search = Paragraph::new(search_text)
            .block(Block::default().borders(Borders::ALL).title("Search"));
        Widget::render(search, chunks[1], buf);

        // Extension list
        let items: Vec<ListItem<'_>> = self
            .active_list()
            .iter()
            .enumerate()
            .map(|(i, ext)| {
                let status = if ext.installed {
                    if ext.enabled { "[Installed]" } else { "[Disabled]" }
                } else {
                    "[Install]"
                };
                let line = format!(
                    "{} {} — {} v{}: {}",
                    status, ext.name, ext.publisher, ext.version, ext.description
                );
                let style = if i == self.selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(line, style))
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Extensions"));
        Widget::render(list, chunks[2], buf);
    }
}

impl Default for ExtensionBrowserView {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionBrowserView {
    /// Returns the currently selected extension, if any.
    pub fn selected_extension(&self) -> Option<&ExtensionInfo> {
        self.active_list().get(self.selected)
    }

    /// Clears the search query and results, switching back to Installed tab.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.active_tab = ExtensionTab::Installed;
        self.selected = 0;
    }

    /// Number of installed extensions.
    pub fn installed_count(&self) -> usize {
        self.installed.len()
    }

    /// Number of search results.
    pub fn search_count(&self) -> usize {
        self.search_results.len()
    }

    /// Removes an installed extension by id, returning `true` if found.
    pub fn remove_installed(&mut self, id: &str) -> bool {
        let before = self.installed.len();
        self.installed.retain(|e| e.id != id);
        self.installed.len() < before
    }
}

// ---------------------------------------------------------------------------
// ViewRegistry – typed panel management
// ---------------------------------------------------------------------------

/// Registry that tracks panels by id and supports type-based lookups.
pub struct ViewRegistry {
    panels: Vec<WebviewPanel>,
}

impl ViewRegistry {
    pub fn new() -> Self {
        Self { panels: Vec::new() }
    }

    pub fn register(&mut self, panel: WebviewPanel) {
        if !self.panels.iter().any(|p| p.id == panel.id) {
            self.panels.push(panel);
        }
    }

    pub fn unregister(&mut self, id: &str) -> Option<WebviewPanel> {
        if let Some(idx) = self.panels.iter().position(|p| p.id == id) {
            Some(self.panels.remove(idx))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&WebviewPanel> {
        self.panels.iter().find(|p| p.id == id)
    }

    pub fn list(&self) -> &[WebviewPanel] {
        &self.panels
    }

    pub fn find_by_type(&self, view_type: &str) -> Vec<&WebviewPanel> {
        self.panels.iter().filter(|p| p.view_type == view_type).collect()
    }

    pub fn len(&self) -> usize {
        self.panels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }

    pub fn visible_panels(&self) -> Vec<&WebviewPanel> {
        self.panels.iter().filter(|p| p.is_visible).collect()
    }

    /// Removes all panels from the registry, returning the count removed.
    pub fn clear(&mut self) -> usize {
        let n = self.panels.len();
        self.panels.clear();
        n
    }

    /// Returns the ids of all registered panels.
    pub fn ids(&self) -> Vec<&str> {
        self.panels.iter().map(|p| p.id.as_str()).collect()
    }

    /// Finds panels whose title contains `query` (case-insensitive).
    pub fn find_by_title(&self, query: &str) -> Vec<&WebviewPanel> {
        let q = query.to_lowercase();
        self.panels
            .iter()
            .filter(|p| p.title.to_lowercase().contains(&q))
            .collect()
    }

    /// Retains only panels matching the predicate.
    pub fn retain(&mut self, f: impl Fn(&WebviewPanel) -> bool) {
        self.panels.retain(|p| f(p));
    }
}

// ---------------------------------------------------------------------------
// ViewMessageRouter – dispatch messages to handlers
// ---------------------------------------------------------------------------

/// A handler function type for view messages.
pub type ViewMessageHandler = Box<dyn Fn(&ViewMessage) -> Option<serde_json::Value>>;

/// Routes ViewMessages to registered handler functions by message type name.
pub struct ViewMessageRouter {
    handlers: Vec<(String, ViewMessageHandler)>,
}

impl ViewMessageRouter {
    pub fn new() -> Self {
        Self { handlers: Vec::new() }
    }

    /// Register a handler for a specific message type name.
    pub fn register_handler(&mut self, msg_type: impl Into<String>, handler: ViewMessageHandler) {
        self.handlers.push((msg_type.into(), handler));
    }

    fn message_type_name(msg: &ViewMessage) -> &'static str {
        match msg {
            ViewMessage::CreateWebviewPanel { .. } => "CreateWebviewPanel",
            ViewMessage::DisposePanel { .. } => "DisposePanel",
            ViewMessage::RevealPanel { .. } => "RevealPanel",
            ViewMessage::SetTitle { .. } => "SetTitle",
            ViewMessage::SetHtml { .. } => "SetHtml",
        }
    }

    /// Dispatch a message. Returns the response from the first matching handler.
    pub fn dispatch(&self, msg: &ViewMessage) -> Option<serde_json::Value> {
        let type_name = Self::message_type_name(msg);
        for (pattern, handler) in &self.handlers {
            if pattern == type_name {
                return handler(msg);
            }
        }
        None
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

// ---------------------------------------------------------------------------
// ViewLayoutManager – track panel positions
// ---------------------------------------------------------------------------

/// Tracks panel layout across columns with visibility state.
pub struct ViewLayoutManager {
    column_panels: Vec<Vec<String>>,
    hidden: Vec<String>,
}

impl ViewLayoutManager {
    /// Create a layout manager with the given number of columns.
    pub fn new(columns: usize) -> Self {
        Self {
            column_panels: vec![Vec::new(); columns],
            hidden: Vec::new(),
        }
    }

    /// Assign a panel id to a column (0-indexed).
    pub fn assign(&mut self, panel_id: impl Into<String>, column: usize) {
        let id = panel_id.into();
        self.remove_panel(&id);
        if column < self.column_panels.len() {
            self.column_panels[column].push(id);
        }
    }

    /// Hide a panel (remove from column, add to hidden list).
    pub fn hide(&mut self, panel_id: &str) {
        for col in &mut self.column_panels {
            col.retain(|id| id != panel_id);
        }
        if !self.hidden.contains(&panel_id.to_string()) {
            self.hidden.push(panel_id.to_string());
        }
    }

    /// Show a hidden panel, placing it in the given column.
    pub fn show(&mut self, panel_id: &str, column: usize) {
        self.hidden.retain(|id| id != panel_id);
        self.assign(panel_id, column);
    }

    /// Get panels in a specific column.
    pub fn panels_in_column(&self, column: usize) -> &[String] {
        self.column_panels.get(column).map_or(&[], |v| v.as_slice())
    }

    /// Number of columns.
    pub fn column_count(&self) -> usize {
        self.column_panels.len()
    }

    /// Total visible panels.
    pub fn visible_count(&self) -> usize {
        self.column_panels.iter().map(|c| c.len()).sum()
    }

    /// Total hidden panels.
    pub fn hidden_count(&self) -> usize {
        self.hidden.len()
    }

    fn remove_panel(&mut self, id: &str) {
        for col in &mut self.column_panels {
            col.retain(|pid| pid != id);
        }
        self.hidden.retain(|pid| pid != id);
    }

    /// Check if a panel is visible (in any column).
    pub fn is_panel_visible(&self, panel_id: &str) -> bool {
        self.column_panels.iter().any(|col| col.iter().any(|id| id == panel_id))
    }

    /// Total number of panels (visible + hidden).
    pub fn total_count(&self) -> usize {
        self.visible_count() + self.hidden_count()
    }

    /// Returns the 0-based column index a panel resides in, or `None`.
    pub fn find_panel_column(&self, panel_id: &str) -> Option<usize> {
        self.column_panels
            .iter()
            .position(|col| col.iter().any(|id| id == panel_id))
    }

    /// Swaps two panels across their respective columns.  Returns `true` if
    /// both panels were found and swapped.
    pub fn swap_panels(&mut self, a: &str, b: &str) -> bool {
        let col_a = self.find_panel_column(a);
        let col_b = self.find_panel_column(b);
        match (col_a, col_b) {
            (Some(ca), Some(cb)) => {
                if let Some(pos_a) = self.column_panels[ca].iter().position(|id| id == a) {
                    if let Some(pos_b) = self.column_panels[cb].iter().position(|id| id == b) {
                        // Swap the ids in-place.  For same-column swaps this
                        // works because we have distinct positions.
                        if ca == cb {
                            self.column_panels[ca].swap(pos_a, pos_b);
                        } else {
                            self.column_panels[ca][pos_a] = b.to_string();
                            self.column_panels[cb][pos_b] = a.to_string();
                        }
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}


// ── View Serializer ──

/// Provides JSON serialization and deserialization for view types.
pub struct ViewSerializer;

impl ViewSerializer {
    /// Serialize a `WebviewPanel` to a JSON string.
    pub fn serialize_panel(panel: &WebviewPanel) -> Result<String, serde_json::Error> {
        serde_json::to_string(panel)
    }

    /// Serialize a `WebviewPanel` to a pretty-printed JSON string.
    pub fn serialize_panel_pretty(panel: &WebviewPanel) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(panel)
    }

    /// Deserialize a `WebviewPanel` from a JSON string.
    pub fn deserialize_panel(json: &str) -> Result<WebviewPanel, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize a `ViewMessage` to a JSON string.
    pub fn serialize_message(msg: &ViewMessage) -> Result<String, serde_json::Error> {
        serde_json::to_string(msg)
    }

    /// Deserialize a `ViewMessage` from a JSON string.
    pub fn deserialize_message(json: &str) -> Result<ViewMessage, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize a slice of panels into a JSON array string.
    pub fn serialize_panels(panels: &[WebviewPanel]) -> Result<String, serde_json::Error> {
        serde_json::to_string(panels)
    }

    /// Deserialize a JSON array string into a vector of panels.
    pub fn deserialize_panels(json: &str) -> Result<Vec<WebviewPanel>, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize a slice of messages into a JSON array string.
    pub fn serialize_messages(msgs: &[ViewMessage]) -> Result<String, serde_json::Error> {
        serde_json::to_string(msgs)
    }

    /// Deserialize a JSON array string into a vector of messages.
    pub fn deserialize_messages(json: &str) -> Result<Vec<ViewMessage>, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert a `WebviewPanel` to a `serde_json::Value`.
    pub fn panel_to_value(panel: &WebviewPanel) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(panel)
    }

    /// Convert a `serde_json::Value` to a `WebviewPanel`.
    pub fn value_to_panel(value: serde_json::Value) -> Result<WebviewPanel, serde_json::Error> {
        serde_json::from_value(value)
    }
}

// ── Badge Counter ──

/// Tracks badge counts per view identifier.
///
/// Used to display notification badges on view tabs or sidebar items.
pub struct ViewBadgeCounter {
    counts: std::collections::HashMap<String, u32>,
}

impl ViewBadgeCounter {
    /// Create a new empty badge counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Set the badge count for the given view to an exact value.
    pub fn set(&mut self, view_id: &str, count: u32) {
        self.counts.insert(view_id.to_string(), count);
    }

    /// Get the current badge count for the given view, returning 0 if unset.
    pub fn get(&self, view_id: &str) -> u32 {
        self.counts.get(view_id).copied().unwrap_or(0)
    }

    /// Increment the badge count for the given view by one, returning the new value.
    pub fn increment(&mut self, view_id: &str) -> u32 {
        let entry = self.counts.entry(view_id.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    /// Decrement the badge count for the given view by one (saturating at 0).
    /// Returns the new value.
    pub fn decrement(&mut self, view_id: &str) -> u32 {
        let entry = self.counts.entry(view_id.to_string()).or_insert(0);
        *entry = entry.saturating_sub(1);
        *entry
    }

    /// Clear the badge count for a single view, returning the old value.
    pub fn clear(&mut self, view_id: &str) -> u32 {
        self.counts.remove(view_id).unwrap_or(0)
    }

    /// Clear all badge counts.
    pub fn clear_all(&mut self) {
        self.counts.clear();
    }

    /// Return the total badge count across all views.
    pub fn total(&self) -> u32 {
        self.counts.values().sum()
    }

    /// Return the number of views that have a non-zero badge count.
    pub fn active_count(&self) -> usize {
        self.counts.values().filter(|&&v| v > 0).count()
    }

    /// Return true if a view has a non-zero badge count.
    pub fn has_badge(&self, view_id: &str) -> bool {
        self.get(view_id) > 0
    }

    /// Return all view ids that currently have badges, sorted alphabetically.
    pub fn views_with_badges(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .counts
            .iter()
            .filter(|(_, v)| **v > 0)
            .map(|(k, _)| k.clone())
            .collect();
        ids.sort();
        ids
    }
}

impl Default for ViewBadgeCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Context Menu Builder ──

/// A single item in a context menu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContextMenuItem {
    /// A clickable action with a label, command id, and enabled state.
    Action {
        label: String,
        command: String,
        enabled: bool,
    },
    /// A visual separator between groups of items.
    Separator,
    /// A submenu containing nested items.
    SubMenu {
        label: String,
        items: Vec<ContextMenuItem>,
    },
}

impl ContextMenuItem {
    /// Create a new enabled action item.
    pub fn action(label: &str, command: &str) -> Self {
        Self::Action {
            label: label.to_string(),
            command: command.to_string(),
            enabled: true,
        }
    }

    /// Create a new disabled action item.
    pub fn action_disabled(label: &str, command: &str) -> Self {
        Self::Action {
            label: label.to_string(),
            command: command.to_string(),
            enabled: false,
        }
    }

    /// Create a separator.
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Create a submenu with the given items.
    pub fn submenu(label: &str, items: Vec<ContextMenuItem>) -> Self {
        Self::SubMenu {
            label: label.to_string(),
            items,
        }
    }

    /// Returns true if this item is a separator.
    pub fn is_separator(&self) -> bool {
        matches!(self, Self::Separator)
    }

    /// Returns true if this item is a submenu.
    pub fn is_submenu(&self) -> bool {
        matches!(self, Self::SubMenu { .. })
    }

    /// Returns true if this item is an enabled action.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Action { enabled: true, .. })
    }

    /// Recursively count all action items (excluding separators).
    pub fn action_count(&self) -> usize {
        match self {
            Self::Action { .. } => 1,
            Self::Separator => 0,
            Self::SubMenu { items, .. } => items.iter().map(|i| i.action_count()).sum(),
        }
    }
}

impl fmt::Display for ContextMenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Action { label, command, enabled } => {
                if *enabled {
                    write!(f, "{label} ({command})")
                } else {
                    write!(f, "{label} ({command}) [disabled]")
                }
            }
            Self::Separator => write!(f, "---"),
            Self::SubMenu { label, items } => {
                write!(f, "{label} [{} items]", items.len())
            }
        }
    }
}

/// Builder for constructing context menus incrementally.
pub struct ViewContextMenuBuilder {
    items: Vec<ContextMenuItem>,
}

impl ViewContextMenuBuilder {
    /// Create a new empty context menu builder.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add an enabled action to the menu.
    pub fn action(mut self, label: &str, command: &str) -> Self {
        self.items.push(ContextMenuItem::action(label, command));
        self
    }

    /// Add a disabled action to the menu.
    pub fn action_disabled(mut self, label: &str, command: &str) -> Self {
        self.items.push(ContextMenuItem::action_disabled(label, command));
        self
    }

    /// Add a separator to the menu.
    pub fn separator(mut self) -> Self {
        self.items.push(ContextMenuItem::separator());
        self
    }

    /// Add a submenu built via a closure.
    pub fn submenu(mut self, label: &str, build: impl FnOnce(ViewContextMenuBuilder) -> ViewContextMenuBuilder) -> Self {
        let sub = build(ViewContextMenuBuilder::new()).build();
        self.items.push(ContextMenuItem::submenu(label, sub));
        self
    }

    /// Add a pre-built submenu.
    pub fn submenu_items(mut self, label: &str, items: Vec<ContextMenuItem>) -> Self {
        self.items.push(ContextMenuItem::submenu(label, items));
        self
    }

    /// Return the number of top-level items added so far.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return true if no items have been added.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Consume the builder and return the list of menu items.
    pub fn build(self) -> Vec<ContextMenuItem> {
        self.items
    }

    /// Count all action items recursively across all submenus.
    pub fn total_actions(&self) -> usize {
        self.items.iter().map(|i| i.action_count()).sum()
    }
}

impl Default for ViewContextMenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Collapse State ──

/// Tracks which tree nodes are collapsed in a tree view.
///
/// By default nodes are expanded; only collapsed node ids are stored.
pub struct ViewCollapseState {
    collapsed: std::collections::HashSet<String>,
}

impl ViewCollapseState {
    /// Create a new state with all nodes expanded.
    pub fn new() -> Self {
        Self {
            collapsed: std::collections::HashSet::new(),
        }
    }

    /// Collapse a single node. Returns true if it was not already collapsed.
    pub fn collapse(&mut self, node_id: &str) -> bool {
        self.collapsed.insert(node_id.to_string())
    }

    /// Expand a single node. Returns true if it was previously collapsed.
    pub fn expand(&mut self, node_id: &str) -> bool {
        self.collapsed.remove(node_id)
    }

    /// Toggle the collapse state of a node. Returns `true` if the node is
    /// now collapsed, `false` if it is now expanded.
    pub fn toggle(&mut self, node_id: &str) -> bool {
        if self.collapsed.contains(node_id) {
            self.collapsed.remove(node_id);
            false
        } else {
            self.collapsed.insert(node_id.to_string());
            true
        }
    }

    /// Returns true if the given node is currently collapsed.
    pub fn is_collapsed(&self, node_id: &str) -> bool {
        self.collapsed.contains(node_id)
    }

    /// Returns true if the given node is currently expanded.
    pub fn is_expanded(&self, node_id: &str) -> bool {
        !self.is_collapsed(node_id)
    }

    /// Collapse all nodes in the provided list.
    pub fn collapse_all(&mut self, node_ids: &[&str]) {
        for id in node_ids {
            self.collapsed.insert(id.to_string());
        }
    }

    /// Expand all nodes, clearing the collapsed set entirely.
    pub fn expand_all(&mut self) {
        self.collapsed.clear();
    }

    /// Return the number of currently collapsed nodes.
    pub fn collapsed_count(&self) -> usize {
        self.collapsed.len()
    }

    /// Return all collapsed node ids, sorted alphabetically.
    pub fn collapsed_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.collapsed.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Returns true if no nodes are collapsed (all expanded).
    pub fn all_expanded(&self) -> bool {
        self.collapsed.is_empty()
    }
}

impl Default for ViewCollapseState {
    fn default() -> Self {
        Self::new()
    }
}




// ---------------------------------------------------------------------------
// ext_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension view containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtViewExtViewLocation {
    Sidebar,
    Panel,
    Editor,
    ActivityBar,
}

impl YExtViewExtViewLocation {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Sidebar => 0,
            Self::Panel => 1,
            Self::Editor => 2,
            Self::ActivityBar => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sidebar => "Sidebar",
            Self::Panel => "Panel",
            Self::Editor => "Editor",
            Self::ActivityBar => "ActivityBar",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtViewExtViewLocation] {
        &[
            YExtViewExtViewLocation::Sidebar,
            YExtViewExtViewLocation::Panel,
            YExtViewExtViewLocation::Editor,
            YExtViewExtViewLocation::ActivityBar,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtViewExtViewLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks view contribution data.
#[derive(Debug, Clone)]
pub struct YExtViewExtViewContribution {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
}

impl YExtViewExtViewContribution {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            icon: None,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtViewExtViewContribution({}: {:?})", "id", self.id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_view – Extended ext view badge helpers
// ---------------------------------------------------------------------------

/// Priority levels for ext view badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtViewPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExtViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks ext view badge data.
#[derive(Debug, Clone)]
pub struct ZExtViewExtViewBadge {
    pub counts: Vec<(String, u32)>,
    pub visible: bool,
    pub animate: bool,
}

impl ZExtViewExtViewBadge {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            counts: Vec::new(),
            visible: false,
            animate: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.counts.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtViewExtViewBadge[visible={:?}, animate={:?}]", self.visible, self.animate)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.animate = !c.animate;
        c
    }
}

/// Compute a simple rolling hash for ext view badge.
pub fn z_ext_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_view_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_ext_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 76
// ---------------------------------------------------------------------------

/// Generic object pool `Xc76Pool<T>`.
pub struct Xc76Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc76Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc76PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc76Pool<T> {
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
    pub fn stats(&self) -> Xc76PoolStats {
        Xc76PoolStats {
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

impl<T> Default for Xc76Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc76Scheduler`.
pub struct Xc76Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc76Scheduler {
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

impl Default for Xc76Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_76 hash for the given byte slice.
pub fn xc_76_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_76 convention.
pub fn xc_76_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_10 deepening: state machine + event bus ---

/// States for the Xd10 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd10State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd10State {
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
pub struct Xd10Transition {
    pub from: Xd10State,
    pub to: Xd10State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd10StateMachine {
    current: Xd10State,
    history: Vec<Xd10Transition>,
    step_counter: usize,
}

impl Xd10StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd10State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd10State {
        self.current
    }

    pub fn history(&self) -> &[Xd10Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd10State) -> Result<Xd10State, String> {
        let allowed = match (self.current, target) {
            (Xd10State::Idle, Xd10State::Running) => true,
            (Xd10State::Running, Xd10State::Paused) => true,
            (Xd10State::Running, Xd10State::Done) => true,
            (Xd10State::Paused, Xd10State::Running) => true,
            (Xd10State::Paused, Xd10State::Done) => true,
            (Xd10State::Done, Xd10State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_10: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd10Transition {
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
            "Xd10SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd10State> {
        let prefix = "Xd10SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd10State::Idle),
            "Running" => Some(Xd10State::Running),
            "Paused" => Some(Xd10State::Paused),
            "Done" => Some(Xd10State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd10State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd10 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd10Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd10Event {
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

type Xd10HandlerFn = Box<dyn Fn(&Xd10Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd10EventBus {
    handlers: Vec<(usize, Option<String>, Xd10HandlerFn)>,
    next_id: usize,
    published: Vec<Xd10Event>,
}

impl Xd10EventBus {
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
        F: Fn(&Xd10Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd10Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd10Event) {
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

    pub fn published_events(&self) -> &[Xd10Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = ViewMessage::CreateWebviewPanel {
            view_type: "preview".into(),
            title: "Preview".into(),
            column: ViewColumn::Beside,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ViewMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn panel_serialization() {
        let p = WebviewPanel {
            id: "p1".into(),
            view_type: "md".into(),
            title: "README".into(),
            column: ViewColumn::One,
            html: "<h1>Hi</h1>".into(),
            is_visible: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: WebviewPanel = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn bridge_create_and_dispose() {
        let mut bridge = ViewBridge::new();
        let id = bridge.create_panel("md", "README", ViewColumn::One);
        assert!(bridge.get_panel(&id).is_some());
        assert!(bridge.dispose_panel(&id));
        assert!(bridge.get_panel(&id).is_none());
    }

    #[test]
    fn bridge_set_html() {
        let mut bridge = ViewBridge::new();
        let id = bridge.create_panel("md", "README", ViewColumn::One);
        bridge.handle_message(&ViewMessage::SetHtml {
            panel_id: id.clone(),
            html: "<p>hello</p>".into(),
        });
        assert_eq!(bridge.get_panel(&id).unwrap().html, "<p>hello</p>");
    }

    #[test]
    fn bridge_dispose_unknown() {
        let mut bridge = ViewBridge::new();
        assert!(!bridge.dispose_panel("nope"));
    }

    // ── Additional tests ──

    #[test]
    fn view_error_display() {
        let e = ViewError::PanelNotFound("panel-42".into());
        assert_eq!(e.to_string(), "panel not found: panel-42");

        let e = ViewError::InvalidInput("bad title".into());
        assert_eq!(e.to_string(), "invalid input: bad title");

        let e = ViewError::PanelLimitExceeded { limit: 64 };
        assert_eq!(e.to_string(), "panel limit exceeded (max 64)");
    }

    #[test]
    fn view_column_conversions() {
        assert_eq!(ViewColumn::from_number(1), Some(ViewColumn::One));
        assert_eq!(ViewColumn::from_number(2), Some(ViewColumn::Two));
        assert_eq!(ViewColumn::from_number(3), Some(ViewColumn::Three));
        assert_eq!(ViewColumn::from_number(0), None);
        assert_eq!(ViewColumn::from_number(4), None);

        assert_eq!(ViewColumn::One.to_number(), Some(1));
        assert_eq!(ViewColumn::Active.to_number(), None);
        assert!(ViewColumn::Beside.is_symbolic());
        assert!(!ViewColumn::Two.is_symbolic());
    }

    #[test]
    fn view_column_display() {
        assert_eq!(ViewColumn::Active.to_string(), "Active");
        assert_eq!(ViewColumn::One.to_string(), "1");
    }

    #[test]
    fn webview_panel_display_and_helpers() {
        let mut p = WebviewPanel {
            id: "p1".into(),
            view_type: "md".into(),
            title: "Notes".into(),
            column: ViewColumn::Two,
            html: "<b>hi</b>".into(),
            is_visible: true,
        };
        assert!(p.to_string().contains("Notes"));
        assert!(p.has_content());
        assert_eq!(p.html_byte_len(), 9);
        p.clear_html();
        assert!(!p.has_content());
        assert_eq!(p.html_byte_len(), 0);
    }

    #[test]
    fn webview_panel_wrap_in_document() {
        let mut p = WebviewPanel {
            id: "p1".into(),
            view_type: "md".into(),
            title: "T".into(),
            column: ViewColumn::One,
            html: "<p>content</p>".into(),
            is_visible: true,
        };
        p.wrap_in_document();
        assert!(p.html.starts_with("<!DOCTYPE html>"));
        assert!(p.html.contains("<p>content</p>"));

        // Calling again should be a no-op since it already starts with <!DOCTYPE
        let snapshot = p.html.clone();
        p.wrap_in_document();
        assert_eq!(p.html, snapshot);
    }

    #[test]
    fn panel_builder_success() {
        let mut bridge = ViewBridge::new();
        let id = bridge
            .create_from_builder(
                PanelBuilder::new()
                    .view_type("preview")
                    .title("My Panel")
                    .column(ViewColumn::Beside)
                    .html("<p>built</p>"),
            )
            .unwrap();
        let panel = bridge.get_panel(&id).unwrap();
        assert_eq!(panel.title, "My Panel");
        assert_eq!(panel.html, "<p>built</p>");
        assert_eq!(panel.column, ViewColumn::Beside);
    }

    #[test]
    fn panel_builder_validation_errors() {
        let result = PanelBuilder::new().title("T").build();
        assert!(matches!(result, Err(ViewError::InvalidInput(_))));

        let result = PanelBuilder::new().view_type("vt").build();
        assert!(matches!(result, Err(ViewError::InvalidInput(_))));

        let result = PanelBuilder::new().view_type("  ").title("T").build();
        assert!(matches!(result, Err(ViewError::InvalidInput(_))));
    }

    #[test]
    fn bridge_search_and_grouping() {
        let mut bridge = ViewBridge::new();
        bridge.create_panel("md", "Alpha Notes", ViewColumn::One);
        bridge.create_panel("md", "Beta Notes", ViewColumn::Two);
        bridge.create_panel("html", "Gamma Preview", ViewColumn::One);

        let results = bridge.search_panels("notes");
        assert_eq!(results.len(), 2);

        let by_col = bridge.panels_by_column();
        assert_eq!(by_col["1"].len(), 2);
        assert_eq!(by_col["2"].len(), 1);
    }

    #[test]
    fn bridge_move_hide_visible() {
        let mut bridge = ViewBridge::new();
        let id = bridge.create_panel("md", "Test", ViewColumn::One);

        bridge.move_panel(&id, ViewColumn::Three).unwrap();
        assert_eq!(bridge.get_panel(&id).unwrap().column, ViewColumn::Three);

        assert_eq!(bridge.visible_panels().len(), 1);
        bridge.hide_panel(&id).unwrap();
        assert_eq!(bridge.visible_panels().len(), 0);
        assert!(bridge.get_panel(&id).unwrap().is_visible == false);

        assert!(bridge.move_panel("nope", ViewColumn::One).is_err());
        assert!(bridge.hide_panel("nope").is_err());
    }

    #[test]
    fn bridge_dispose_all_and_diagnostics() {
        let mut bridge = ViewBridge::new();
        bridge.create_panel("a", "A", ViewColumn::One);
        bridge.create_panel("b", "B", ViewColumn::Two);
        let id3 = bridge.create_panel("c", "C", ViewColumn::Three);
        bridge.hide_panel(&id3).unwrap();

        let diag = bridge.diagnostics();
        assert_eq!(diag["totalPanels"], 3);
        assert_eq!(diag["visiblePanels"], 2);
        assert_eq!(diag["hiddenPanels"], 1);

        let removed = bridge.dispose_all();
        assert_eq!(removed, 3);
        assert_eq!(bridge.panel_count(), 0);
    }

    #[test]
    fn bridge_reveal_panel_via_message() {
        let mut bridge = ViewBridge::new();
        let id = bridge.create_panel("md", "Doc", ViewColumn::One);
        bridge.hide_panel(&id).unwrap();
        assert!(!bridge.get_panel(&id).unwrap().is_visible);

        let result = bridge.handle_message(&ViewMessage::RevealPanel {
            panel_id: id.clone(),
            column: ViewColumn::Two,
            preserve_focus: true,
        });
        assert_eq!(result["revealed"], true);
        let panel = bridge.get_panel(&id).unwrap();
        assert!(panel.is_visible);
        assert_eq!(panel.column, ViewColumn::Two);
    }

    // ── Extensions View Tests ──

    fn make_installed_ext(id: &str, name: &str, publisher: &str, version: &str, enabled: bool) -> InstalledExtension {
        InstalledExtension {
            id: id.into(),
            version: version.into(),
            path: format!("/ext/{id}"),
            is_enabled: enabled,
            manifest: vsedit_ext_mgmt::ExtensionManifest {
                name: id.split('.').last().unwrap_or(id).into(),
                display_name: name.into(),
                publisher: publisher.into(),
                version: version.into(),
                description: format!("Description of {name}"),
                contributes: vsedit_ext_mgmt::ExtensionContributions::default(),
                extension_dependencies: Vec::new(),
            },
        }
    }

    fn make_gallery_ext(id: &str, name: &str, publisher: &str) -> GalleryExtension {
        GalleryExtension {
            id: id.into(),
            display_name: name.into(),
            publisher: publisher.into(),
            version: "1.0.0".into(),
            description: format!("Gallery {name}"),
            download_count: 1000,
            rating: 4.5,
            install_count: 1000,
            download_url: None,
        }
    }

    #[test]
    fn extensions_tab_default() {
        assert_eq!(ExtensionsTab::default(), ExtensionsTab::Installed);
    }

    #[test]
    fn extensions_tab_display() {
        assert_eq!(ExtensionsTab::Installed.to_string(), "Installed");
        assert_eq!(ExtensionsTab::Recommended.to_string(), "Recommended");
        assert_eq!(ExtensionsTab::SearchResults.to_string(), "Search Results");
    }

    #[test]
    fn extension_list_item_from_installed() {
        let ext = make_installed_ext("pub.ext", "My Ext", "pub", "1.0.0", true);
        let item = ExtensionListItem::from_installed(&ext);
        assert_eq!(item.id, "pub.ext");
        assert_eq!(item.name, "My Ext");
        assert!(item.is_installed);
        assert!(item.is_enabled);
    }

    #[test]
    fn extension_list_item_from_gallery() {
        let ext = make_gallery_ext("pub.ext", "Gallery Ext", "pub");
        let item = ExtensionListItem::from_gallery(&ext);
        assert_eq!(item.id, "pub.ext");
        assert!(!item.is_installed);
        assert!(!item.is_enabled);
    }

    #[test]
    fn extension_list_item_render_line() {
        let ext = make_installed_ext("pub.ext", "My Ext", "pub", "1.0.0", true);
        let item = ExtensionListItem::from_installed(&ext);
        let line = item.render_line();
        assert!(line.contains("✓"));
        assert!(line.contains("My Ext"));
        assert!(line.contains("v1.0.0"));

        let disabled = make_installed_ext("pub.ext", "Disabled Ext", "pub", "2.0.0", false);
        let item2 = ExtensionListItem::from_installed(&disabled);
        assert!(item2.render_line().contains("○"));
    }

    #[test]
    fn extension_detail_from_installed() {
        let mut ext = make_installed_ext("pub.ext", "My Ext", "pub", "1.0.0", true);
        ext.manifest.extension_dependencies = vec!["dep.one".into()];
        let detail = ExtensionDetailView::from_installed(&ext);
        assert_eq!(detail.id, "pub.ext");
        assert!(detail.is_installed);
        assert_eq!(detail.dependencies.len(), 1);
    }

    #[test]
    fn extension_detail_render() {
        let ext = make_installed_ext("pub.ext", "My Ext", "pub", "1.0.0", true);
        let detail = ExtensionDetailView::from_installed(&ext);
        let rendered = detail.render();
        assert!(rendered.contains("My Ext"));
        assert!(rendered.contains("Enabled"));
        assert!(rendered.contains("Dependencies: None"));
    }

    #[test]
    fn extensions_view_state_new() {
        let state = ExtensionsViewState::new();
        assert_eq!(state.active_tab, ExtensionsTab::Installed);
        assert!(state.installed_items.is_empty());
        assert!(state.search_query.is_empty());
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn extensions_view_state_load_installed() {
        let mut state = ExtensionsViewState::new();
        let exts = vec![
            make_installed_ext("pub.ext1", "Ext 1", "pub", "1.0.0", true),
            make_installed_ext("pub.ext2", "Ext 2", "pub", "2.0.0", false),
        ];
        state.load_installed(&exts);
        assert_eq!(state.installed_items.len(), 2);
        assert_eq!(state.item_count(), 2);
    }

    #[test]
    fn extensions_view_state_switch_tab() {
        let mut state = ExtensionsViewState::new();
        state.switch_tab(ExtensionsTab::Recommended);
        assert_eq!(state.active_tab, ExtensionsTab::Recommended);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn extensions_view_state_search() {
        let mut state = ExtensionsViewState::new();
        let results = vec![make_gallery_ext("pub.ext", "Result", "pub")];
        state.load_search_results(&results);
        state.set_search_query("rust");
        assert_eq!(state.active_tab, ExtensionsTab::SearchResults);
        assert_eq!(state.search_query, "rust");
        assert_eq!(state.item_count(), 1);
    }

    #[test]
    fn extensions_view_state_navigation() {
        let mut state = ExtensionsViewState::new();
        let exts = vec![
            make_installed_ext("pub.ext1", "Ext 1", "pub", "1.0.0", true),
            make_installed_ext("pub.ext2", "Ext 2", "pub", "2.0.0", true),
            make_installed_ext("pub.ext3", "Ext 3", "pub", "3.0.0", true),
        ];
        state.load_installed(&exts);
        assert_eq!(state.selected_index, 0);

        state.select_next();
        assert_eq!(state.selected_index, 1);
        state.select_next();
        assert_eq!(state.selected_index, 2);
        state.select_next(); // should not go past end
        assert_eq!(state.selected_index, 2);

        state.select_prev();
        assert_eq!(state.selected_index, 1);
        state.select_prev();
        assert_eq!(state.selected_index, 0);
        state.select_prev(); // should not go below 0
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn extensions_view_state_render_list() {
        let mut state = ExtensionsViewState::new();
        let exts = vec![
            make_installed_ext("pub.ext1", "Ext 1", "pub", "1.0.0", true),
            make_installed_ext("pub.ext2", "Ext 2", "pub", "2.0.0", true),
        ];
        state.load_installed(&exts);
        let lines = state.render_list();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("▸ ")); // selected
        assert!(lines[1].starts_with("  ")); // not selected
    }

    #[test]
    fn extensions_view_state_render_tab_bar() {
        let state = ExtensionsViewState::new();
        let bar = state.render_tab_bar();
        assert!(bar.contains("[Installed]"));
        assert!(bar.contains(" Recommended "));
        assert!(bar.contains(" Search Results "));
    }

    #[test]
    fn extensions_view_state_selected_item() {
        let mut state = ExtensionsViewState::new();
        assert!(state.selected_item().is_none());

        let exts = vec![make_installed_ext("pub.ext1", "Ext 1", "pub", "1.0.0", true)];
        state.load_installed(&exts);
        assert_eq!(state.selected_item().unwrap().id, "pub.ext1");
    }

    #[test]
    fn extension_list_item_serialization() {
        let item = ExtensionListItem {
            id: "pub.ext".into(),
            name: "Ext".into(),
            publisher: "pub".into(),
            version: "1.0.0".into(),
            icon: Some("icon.png".into()),
            is_enabled: true,
            is_installed: true,
            description: "desc".into(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: ExtensionListItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn extension_detail_view_serialization() {
        let detail = ExtensionDetailView {
            id: "pub.ext".into(),
            name: "Ext".into(),
            publisher: "pub".into(),
            version: "1.0.0".into(),
            description: "desc".into(),
            is_installed: true,
            is_enabled: true,
            dependencies: vec!["dep.one".into()],
        };
        let json = serde_json::to_string(&detail).unwrap();
        let back: ExtensionDetailView = serde_json::from_str(&json).unwrap();
        assert_eq!(detail, back);
    }

    // ── ExtensionBrowserView tests ──

    fn make_ext_info(name: &str, installed: bool, enabled: bool) -> ExtensionInfo {
        ExtensionInfo {
            id: format!("pub.{}", name.to_lowercase().replace(' ', "-")),
            name: name.into(),
            publisher: "test-pub".into(),
            description: format!("Desc of {name}"),
            version: "1.0.0".into(),
            installed,
            enabled,
        }
    }

    #[test]
    fn browser_view_new_defaults() {
        let view = ExtensionBrowserView::new();
        assert!(view.installed.is_empty());
        assert!(view.search_results.is_empty());
        assert!(view.search_query.is_empty());
        assert_eq!(view.selected, 0);
        assert_eq!(view.active_tab, ExtensionTab::Installed);
    }

    #[test]
    fn browser_view_add_installed() {
        let mut view = ExtensionBrowserView::new();
        view.add_installed(make_ext_info("Rust Analyzer", true, true));
        view.add_installed(make_ext_info("Prettier", true, false));
        assert_eq!(view.installed.len(), 2);
        assert_eq!(view.active_list().len(), 2);
    }

    #[test]
    fn browser_view_set_search_results() {
        let mut view = ExtensionBrowserView::new();
        let results = vec![
            make_ext_info("Theme A", false, false),
            make_ext_info("Theme B", false, false),
        ];
        view.set_search_results(results);
        assert_eq!(view.active_tab, ExtensionTab::Search);
        assert_eq!(view.search_results.len(), 2);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn browser_view_move_selection_down_and_up() {
        let mut view = ExtensionBrowserView::new();
        view.add_installed(make_ext_info("A", true, true));
        view.add_installed(make_ext_info("B", true, true));
        view.add_installed(make_ext_info("C", true, true));

        assert_eq!(view.selected, 0);
        view.move_selection_down();
        assert_eq!(view.selected, 1);
        view.move_selection_down();
        assert_eq!(view.selected, 2);
        view.move_selection_down(); // at end, stays
        assert_eq!(view.selected, 2);

        view.move_selection_up();
        assert_eq!(view.selected, 1);
        view.move_selection_up();
        assert_eq!(view.selected, 0);
        view.move_selection_up(); // at start, stays
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn browser_view_active_list_switches_with_tab() {
        let mut view = ExtensionBrowserView::new();
        view.add_installed(make_ext_info("Inst", true, true));
        let results = vec![make_ext_info("Search1", false, false)];
        view.set_search_results(results);

        assert_eq!(view.active_tab, ExtensionTab::Search);
        assert_eq!(view.active_list().len(), 1);
        assert_eq!(view.active_list()[0].name, "Search1");

        view.active_tab = ExtensionTab::Installed;
        assert_eq!(view.active_list().len(), 1);
        assert_eq!(view.active_list()[0].name, "Inst");
    }

    #[test]
    fn browser_view_render_does_not_panic() {
        let mut view = ExtensionBrowserView::new();
        view.add_installed(make_ext_info("MyExt", true, true));
        view.search_query = "test".into();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
    }

    #[test]
    fn browser_view_render_empty_does_not_panic() {
        let view = ExtensionBrowserView::new();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
    }

    #[test]
    fn browser_view_selection_on_empty_list() {
        let mut view = ExtensionBrowserView::new();
        view.move_selection_down();
        assert_eq!(view.selected, 0);
        view.move_selection_up();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn browser_view_default_impl() {
        let view = ExtensionBrowserView::default();
        assert_eq!(view.active_tab, ExtensionTab::Installed);
    }

    fn make_panel(id: &str, vtype: &str, col: ViewColumn) -> WebviewPanel {
        WebviewPanel {
            id: id.to_string(),
            view_type: vtype.to_string(),
            title: id.to_string(),
            column: col,
            html: String::new(),
            is_visible: true,
        }
    }

    #[test]
    fn test_view_registry_register_and_find() {
        let mut reg = ViewRegistry::new();
        reg.register(make_panel("p1", "markdown", ViewColumn::One));
        reg.register(make_panel("p2", "terminal", ViewColumn::Two));
        assert_eq!(reg.len(), 2);
        assert!(reg.get("p1").is_some());
        assert_eq!(reg.find_by_type("markdown").len(), 1);
    }

    #[test]
    fn test_view_registry_unregister() {
        let mut reg = ViewRegistry::new();
        reg.register(make_panel("p1", "md", ViewColumn::One));
        let removed = reg.unregister("p1");
        assert!(removed.is_some());
        assert!(reg.is_empty());
        assert!(reg.unregister("p1").is_none());
    }

    #[test]
    fn test_view_message_router_dispatch() {
        let mut router = ViewMessageRouter::new();
        router.register_handler("SetTitle", Box::new(|_msg| {
            Some(serde_json::json!({"reply": "ok"}))
        }));
        let msg = ViewMessage::SetTitle {
            panel_id: "p1".into(),
            title: "New Title".into(),
        };
        let resp = router.dispatch(&msg);
        assert!(resp.is_some());
        assert_eq!(resp.unwrap()["reply"], "ok");
    }

    #[test]
    fn test_view_message_router_no_match() {
        let router = ViewMessageRouter::new();
        let msg = ViewMessage::DisposePanel { panel_id: "p1".into() };
        assert!(router.dispatch(&msg).is_none());
    }

    #[test]
    fn test_view_layout_manager_assign_and_hide() {
        let mut layout = ViewLayoutManager::new(3);
        layout.assign("p1", 0);
        layout.assign("p2", 1);
        assert_eq!(layout.visible_count(), 2);
        assert!(layout.is_panel_visible("p1"));
        layout.hide("p1");
        assert!(!layout.is_panel_visible("p1"));
        assert_eq!(layout.hidden_count(), 1);
    }

    #[test]
    fn test_view_layout_manager_show() {
        let mut layout = ViewLayoutManager::new(2);
        layout.assign("p1", 0);
        layout.hide("p1");
        assert_eq!(layout.visible_count(), 0);
        layout.show("p1", 1);
        assert!(layout.is_panel_visible("p1"));
        assert_eq!(layout.panels_in_column(1), &["p1".to_string()]);
    }

    // ── New deep tests ──

    #[test]
    fn view_column_numeric_helpers() {
        assert!(ViewColumn::One.is_numeric());
        assert!(!ViewColumn::Active.is_numeric());
        let all = ViewColumn::all_numeric();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], ViewColumn::One);
    }

    #[test]
    fn view_column_next_prev() {
        assert_eq!(ViewColumn::One.next_column(), ViewColumn::Two);
        assert_eq!(ViewColumn::Two.next_column(), ViewColumn::Three);
        assert_eq!(ViewColumn::Three.next_column(), ViewColumn::One);
        assert_eq!(ViewColumn::Active.next_column(), ViewColumn::One);

        assert_eq!(ViewColumn::One.prev_column(), ViewColumn::Three);
        assert_eq!(ViewColumn::Two.prev_column(), ViewColumn::One);
        assert_eq!(ViewColumn::Three.prev_column(), ViewColumn::Two);
        assert_eq!(ViewColumn::Beside.prev_column(), ViewColumn::One);
    }

    #[test]
    fn webview_panel_rename_and_filter() {
        let mut p = make_panel("p1", "markdown", ViewColumn::One);
        p.title = "My Notes".into();

        let old = p.rename("Updated Title");
        assert_eq!(old, "My Notes");
        assert_eq!(p.title, "Updated Title");

        assert!(p.matches_filter("updated"));
        assert!(p.matches_filter("MARKDOWN"));
        assert!(!p.matches_filter("html"));
    }

    #[test]
    fn webview_panel_is_type_and_summary() {
        let p = make_panel("p1", "terminal", ViewColumn::Two);
        assert!(p.is_type("terminal"));
        assert!(!p.is_type("markdown"));
        assert_eq!(p.summary(), "p1: p1 (terminal)");
    }

    #[test]
    fn webview_panel_set_visibility() {
        let mut p = make_panel("p1", "md", ViewColumn::One);
        assert!(p.is_visible);
        let was = p.set_visibility(false);
        assert!(was);
        assert!(!p.is_visible);
        let was2 = p.set_visibility(true);
        assert!(!was2);
        assert!(p.is_visible);
    }

    #[test]
    fn view_error_predicates() {
        let nf = ViewError::PanelNotFound("p42".into());
        assert!(nf.is_not_found());
        assert!(!nf.is_invalid_input());
        assert!(!nf.is_limit_exceeded());
        assert_eq!(nf.panel_id(), Some("p42"));

        let inv = ViewError::InvalidInput("bad".into());
        assert!(inv.is_invalid_input());
        assert!(inv.panel_id().is_none());

        let lim = ViewError::PanelLimitExceeded { limit: 10 };
        assert!(lim.is_limit_exceeded());
    }

    #[test]
    fn extension_list_item_qualified_name_and_query() {
        let item = ExtensionListItem {
            id: "pub.rust".into(),
            name: "Rust Analyzer".into(),
            publisher: "matklad".into(),
            version: "0.3.0".into(),
            icon: None,
            is_enabled: true,
            is_installed: true,
            description: "Language server".into(),
        };
        assert_eq!(item.qualified_name(), "matklad.Rust Analyzer");
        assert!(item.matches_query("rust"));
        assert!(item.matches_query("MATKLAD"));
        assert!(item.matches_query("language"));
        assert!(!item.matches_query("python"));
    }

    #[test]
    fn extension_list_item_toggle_and_status() {
        let mut item = ExtensionListItem {
            id: "x".into(),
            name: "X".into(),
            publisher: "p".into(),
            version: "1.0.0".into(),
            icon: None,
            is_enabled: true,
            is_installed: true,
            description: String::new(),
        };
        assert_eq!(item.status_label(), "Enabled");
        let now = item.toggle_enabled();
        assert!(!now);
        assert_eq!(item.status_label(), "Disabled");

        item.is_installed = false;
        assert_eq!(item.status_label(), "Not Installed");
    }

    #[test]
    fn extension_detail_dependencies_and_status() {
        let mut detail = ExtensionDetailView {
            id: "a".into(),
            name: "A".into(),
            publisher: "p".into(),
            version: "1.0.0".into(),
            description: "desc".into(),
            is_installed: true,
            is_enabled: false,
            dependencies: vec!["dep1".into(), "dep2".into()],
        };
        assert!(detail.has_dependencies());
        assert_eq!(detail.dependency_count(), 2);
        assert_eq!(detail.status_label(), "Disabled");
        assert!(detail.matches_query("desc"));

        detail.dependencies.clear();
        assert!(!detail.has_dependencies());
        assert_eq!(detail.dependency_count(), 0);
    }

    #[test]
    fn extensions_view_state_clear_and_find() {
        let mut state = ExtensionsViewState::new();
        let exts = vec![
            make_installed_ext("pub.ext1", "Ext 1", "pub", "1.0.0", true),
            make_installed_ext("pub.ext2", "Ext 2", "pub", "2.0.0", false),
        ];
        state.load_installed(&exts);
        state.set_search_query("hello");
        assert!(state.has_search());

        state.clear_search();
        assert!(!state.has_search());
        assert_eq!(state.active_tab, ExtensionsTab::Installed);
        assert!(state.search_results.is_empty());

        assert_eq!(state.enabled_count(), 1);
        assert_eq!(state.disabled_count(), 1);
        assert!(state.find_by_id("pub.ext1").is_some());
        assert!(state.find_by_id("nonexistent").is_none());
    }

    #[test]
    fn view_registry_clear_ids_find_retain() {
        let mut reg = ViewRegistry::new();
        reg.register(make_panel("p1", "md", ViewColumn::One));
        reg.register(make_panel("p2", "terminal", ViewColumn::Two));
        reg.register(make_panel("p3", "md", ViewColumn::One));

        let ids = reg.ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"p1"));

        // Duplicate register is a no-op
        reg.register(make_panel("p1", "md", ViewColumn::One));
        assert_eq!(reg.len(), 3);

        assert_eq!(reg.find_by_title("p2").len(), 1);

        reg.retain(|p| p.view_type == "md");
        assert_eq!(reg.len(), 2);

        let cleared = reg.clear();
        assert_eq!(cleared, 2);
        assert!(reg.is_empty());
    }

    #[test]
    fn view_layout_total_and_find_column() {
        let mut layout = ViewLayoutManager::new(3);
        layout.assign("p1", 0);
        layout.assign("p2", 2);
        layout.hide("p3");
        assert_eq!(layout.total_count(), 3);
        assert_eq!(layout.find_panel_column("p1"), Some(0));
        assert_eq!(layout.find_panel_column("p2"), Some(2));
        assert_eq!(layout.find_panel_column("p3"), None);
        assert_eq!(layout.find_panel_column("missing"), None);
    }

    #[test]
    fn view_layout_swap_panels() {
        let mut layout = ViewLayoutManager::new(3);
        layout.assign("p1", 0);
        layout.assign("p2", 2);
        assert!(layout.swap_panels("p1", "p2"));
        assert_eq!(layout.find_panel_column("p1"), Some(2));
        assert_eq!(layout.find_panel_column("p2"), Some(0));

        // swap with unknown panel returns false
        assert!(!layout.swap_panels("p1", "unknown"));
    }

    #[test]
    fn browser_view_selected_and_clear() {
        let mut view = ExtensionBrowserView::new();
        assert!(view.selected_extension().is_none());

        view.add_installed(make_ext_info("A", true, true));
        view.add_installed(make_ext_info("B", true, false));
        assert_eq!(view.installed_count(), 2);
        assert_eq!(view.selected_extension().unwrap().name, "A");

        view.set_search_results(vec![make_ext_info("S1", false, false)]);
        assert_eq!(view.search_count(), 1);

        view.clear_search();
        assert_eq!(view.active_tab, ExtensionTab::Installed);
        assert!(view.search_query.is_empty());
        assert_eq!(view.search_count(), 0);
    }

    #[test]
    fn browser_view_remove_installed() {
        let mut view = ExtensionBrowserView::new();
        view.add_installed(make_ext_info("Keep", true, true));
        view.add_installed(make_ext_info("Remove", true, true));
        assert_eq!(view.installed_count(), 2);
        assert!(view.remove_installed("pub.remove"));
        assert_eq!(view.installed_count(), 1);
        assert!(!view.remove_installed("pub.remove")); // already gone
    }

    // ── ViewSerializer tests ──

    #[test]
    fn serializer_panel_roundtrip() {
        let panel = WebviewPanel {
            id: "p-1".into(),
            view_type: "markdown".into(),
            title: "Notes".into(),
            column: ViewColumn::Two,
            html: "<h1>Hi</h1>".into(),
            is_visible: true,
        };
        let json = ViewSerializer::serialize_panel(&panel).unwrap();
        let back = ViewSerializer::deserialize_panel(&json).unwrap();
        assert_eq!(panel, back);
    }

    #[test]
    fn serializer_panel_pretty() {
        let panel = WebviewPanel {
            id: "p-2".into(),
            view_type: "preview".into(),
            title: "Preview".into(),
            column: ViewColumn::Active,
            html: "".into(),
            is_visible: false,
        };
        let pretty = ViewSerializer::serialize_panel_pretty(&panel).unwrap();
        assert!(pretty.contains('\n'), "pretty output should have newlines");
        let back = ViewSerializer::deserialize_panel(&pretty).unwrap();
        assert_eq!(panel, back);
    }

    #[test]
    fn serializer_message_roundtrip() {
        let msg = ViewMessage::SetHtml {
            panel_id: "p-1".into(),
            html: "<p>test</p>".into(),
        };
        let json = ViewSerializer::serialize_message(&msg).unwrap();
        let back = ViewSerializer::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serializer_panels_batch() {
        let panels = vec![
            WebviewPanel {
                id: "a".into(),
                view_type: "t".into(),
                title: "A".into(),
                column: ViewColumn::One,
                html: "".into(),
                is_visible: true,
            },
            WebviewPanel {
                id: "b".into(),
                view_type: "t".into(),
                title: "B".into(),
                column: ViewColumn::Three,
                html: "<b>bold</b>".into(),
                is_visible: false,
            },
        ];
        let json = ViewSerializer::serialize_panels(&panels).unwrap();
        let back = ViewSerializer::deserialize_panels(&json).unwrap();
        assert_eq!(panels, back);
    }

    #[test]
    fn serializer_messages_batch() {
        let msgs = vec![
            ViewMessage::CreateWebviewPanel {
                view_type: "md".into(),
                title: "MD".into(),
                column: ViewColumn::Beside,
            },
            ViewMessage::DisposePanel {
                panel_id: "x".into(),
            },
        ];
        let json = ViewSerializer::serialize_messages(&msgs).unwrap();
        let back = ViewSerializer::deserialize_messages(&json).unwrap();
        assert_eq!(msgs, back);
    }

    #[test]
    fn serializer_panel_to_value() {
        let panel = WebviewPanel {
            id: "v1".into(),
            view_type: "html".into(),
            title: "Val".into(),
            column: ViewColumn::One,
            html: "".into(),
            is_visible: true,
        };
        let val = ViewSerializer::panel_to_value(&panel).unwrap();
        assert_eq!(val["id"], "v1");
        assert_eq!(val["is_visible"], true);
        let back = ViewSerializer::value_to_panel(val).unwrap();
        assert_eq!(panel, back);
    }

    #[test]
    fn serializer_invalid_json() {
        assert!(ViewSerializer::deserialize_panel("not json").is_err());
        assert!(ViewSerializer::deserialize_message("{\"bad\": 1}").is_err());
    }

    // ── ViewBadgeCounter tests ──

    #[test]
    fn badge_set_and_get() {
        let mut bc = ViewBadgeCounter::new();
        assert_eq!(bc.get("explorer"), 0);
        bc.set("explorer", 5);
        assert_eq!(bc.get("explorer"), 5);
        bc.set("explorer", 0);
        assert_eq!(bc.get("explorer"), 0);
    }

    #[test]
    fn badge_increment() {
        let mut bc = ViewBadgeCounter::new();
        assert_eq!(bc.increment("git"), 1);
        assert_eq!(bc.increment("git"), 2);
        assert_eq!(bc.increment("git"), 3);
        assert_eq!(bc.get("git"), 3);
    }

    #[test]
    fn badge_decrement_saturates() {
        let mut bc = ViewBadgeCounter::new();
        assert_eq!(bc.decrement("x"), 0, "decrementing unknown stays at 0");
        bc.set("x", 2);
        assert_eq!(bc.decrement("x"), 1);
        assert_eq!(bc.decrement("x"), 0);
        assert_eq!(bc.decrement("x"), 0, "should saturate at 0");
    }

    #[test]
    fn badge_increment_saturates_at_max() {
        let mut bc = ViewBadgeCounter::new();
        bc.set("overflow", u32::MAX);
        assert_eq!(bc.increment("overflow"), u32::MAX, "saturating_add at max");
    }

    #[test]
    fn badge_clear_single() {
        let mut bc = ViewBadgeCounter::new();
        bc.set("a", 10);
        bc.set("b", 20);
        assert_eq!(bc.clear("a"), 10);
        assert_eq!(bc.get("a"), 0);
        assert_eq!(bc.get("b"), 20);
    }

    #[test]
    fn badge_clear_all() {
        let mut bc = ViewBadgeCounter::new();
        bc.set("a", 5);
        bc.set("b", 3);
        bc.clear_all();
        assert_eq!(bc.total(), 0);
        assert_eq!(bc.active_count(), 0);
    }

    #[test]
    fn badge_total_and_active() {
        let mut bc = ViewBadgeCounter::new();
        bc.set("a", 3);
        bc.set("b", 7);
        bc.set("c", 0);
        assert_eq!(bc.total(), 10);
        assert_eq!(bc.active_count(), 2);
    }

    #[test]
    fn badge_has_badge() {
        let mut bc = ViewBadgeCounter::new();
        assert!(!bc.has_badge("x"));
        bc.increment("x");
        assert!(bc.has_badge("x"));
    }

    #[test]
    fn badge_views_with_badges_sorted() {
        let mut bc = ViewBadgeCounter::new();
        bc.set("zebra", 1);
        bc.set("alpha", 2);
        bc.set("mid", 0);
        let ids = bc.views_with_badges();
        assert_eq!(ids, vec!["alpha", "zebra"]);
    }

    #[test]
    fn badge_default() {
        let bc = ViewBadgeCounter::default();
        assert_eq!(bc.total(), 0);
    }

    // ── ViewContextMenuBuilder tests ──

    #[test]
    fn menu_builder_empty() {
        let builder = ViewContextMenuBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.total_actions(), 0);
        let items = builder.build();
        assert!(items.is_empty());
    }

    #[test]
    fn menu_builder_actions_and_separator() {
        let items = ViewContextMenuBuilder::new()
            .action("Cut", "edit.cut")
            .action("Copy", "edit.copy")
            .separator()
            .action("Paste", "edit.paste")
            .build();
        assert_eq!(items.len(), 4);
        assert_eq!(items[2], ContextMenuItem::Separator);
        assert!(items[0].is_enabled());
        assert!(!items[2].is_separator() == false || items[2].is_separator());
    }

    #[test]
    fn menu_builder_disabled_action() {
        let items = ViewContextMenuBuilder::new()
            .action_disabled("Redo", "edit.redo")
            .build();
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_enabled());
        match &items[0] {
            ContextMenuItem::Action { label, command, enabled } => {
                assert_eq!(label, "Redo");
                assert_eq!(command, "edit.redo");
                assert!(!enabled);
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn menu_builder_submenu_closure() {
        let items = ViewContextMenuBuilder::new()
            .action("Open", "file.open")
            .submenu("Recent", |b| {
                b.action("file1.rs", "open.file1")
                 .action("file2.rs", "open.file2")
                 .separator()
                 .action("Clear", "recent.clear")
            })
            .build();
        assert_eq!(items.len(), 2);
        match &items[1] {
            ContextMenuItem::SubMenu { label, items: sub } => {
                assert_eq!(label, "Recent");
                assert_eq!(sub.len(), 4);
                assert!(sub[2].is_separator());
            }
            _ => panic!("expected SubMenu"),
        }
    }

    #[test]
    fn menu_builder_nested_submenu() {
        let items = ViewContextMenuBuilder::new()
            .submenu("Level1", |b| {
                b.submenu("Level2", |b2| {
                    b2.action("Deep", "cmd.deep")
                })
            })
            .build();
        assert_eq!(items.len(), 1);
        if let ContextMenuItem::SubMenu { items: l1, .. } = &items[0] {
            assert_eq!(l1.len(), 1);
            if let ContextMenuItem::SubMenu { items: l2, .. } = &l1[0] {
                assert_eq!(l2.len(), 1);
                assert!(l2[0].is_enabled());
            } else {
                panic!("expected nested SubMenu");
            }
        } else {
            panic!("expected SubMenu");
        }
    }

    #[test]
    fn menu_total_actions_recursive() {
        let builder = ViewContextMenuBuilder::new()
            .action("A", "a")
            .separator()
            .submenu("Sub", |b| {
                b.action("B", "b")
                 .action("C", "c")
                 .submenu("Deep", |b2| b2.action("D", "d"))
            });
        assert_eq!(builder.total_actions(), 4);
    }

    #[test]
    fn menu_item_display() {
        let action = ContextMenuItem::action("Cut", "edit.cut");
        assert_eq!(format!("{action}"), "Cut (edit.cut)");
        let disabled = ContextMenuItem::action_disabled("Redo", "edit.redo");
        assert_eq!(format!("{disabled}"), "Redo (edit.redo) [disabled]");
        let sep = ContextMenuItem::separator();
        assert_eq!(format!("{sep}"), "---");
        let sub = ContextMenuItem::submenu("More", vec![
            ContextMenuItem::action("X", "x"),
        ]);
        assert_eq!(format!("{sub}"), "More [1 items]");
    }

    #[test]
    fn menu_item_serialization_roundtrip() {
        let items = vec![
            ContextMenuItem::action("Cut", "edit.cut"),
            ContextMenuItem::separator(),
            ContextMenuItem::submenu("Sub", vec![
                ContextMenuItem::action_disabled("Nope", "nope"),
            ]),
        ];
        let json = serde_json::to_string(&items).unwrap();
        let back: Vec<ContextMenuItem> = serde_json::from_str(&json).unwrap();
        assert_eq!(items, back);
    }

    #[test]
    fn menu_submenu_items_method() {
        let pre = vec![ContextMenuItem::action("A", "a")];
        let items = ViewContextMenuBuilder::new()
            .submenu_items("Pre", pre)
            .build();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_submenu());
    }

    #[test]
    fn menu_action_count_on_item() {
        let sep = ContextMenuItem::separator();
        assert_eq!(sep.action_count(), 0);
        let act = ContextMenuItem::action("X", "x");
        assert_eq!(act.action_count(), 1);
        let sub = ContextMenuItem::submenu("S", vec![
            ContextMenuItem::action("A", "a"),
            ContextMenuItem::separator(),
            ContextMenuItem::action("B", "b"),
        ]);
        assert_eq!(sub.action_count(), 2);
    }

    #[test]
    fn menu_default() {
        let builder = ViewContextMenuBuilder::default();
        assert!(builder.is_empty());
    }

    // ── ViewCollapseState tests ──

    #[test]
    fn collapse_initial_all_expanded() {
        let state = ViewCollapseState::new();
        assert!(state.all_expanded());
        assert_eq!(state.collapsed_count(), 0);
        assert!(state.is_expanded("any"));
    }

    #[test]
    fn collapse_and_expand() {
        let mut state = ViewCollapseState::new();
        assert!(state.collapse("node-1"), "first collapse returns true");
        assert!(state.is_collapsed("node-1"));
        assert!(!state.collapse("node-1"), "already collapsed returns false");

        assert!(state.expand("node-1"), "expand returns true");
        assert!(state.is_expanded("node-1"));
        assert!(!state.expand("node-1"), "already expanded returns false");
    }

    #[test]
    fn collapse_toggle() {
        let mut state = ViewCollapseState::new();
        assert!(state.toggle("n"), "toggle expands->collapsed = true");
        assert!(state.is_collapsed("n"));
        assert!(!state.toggle("n"), "toggle collapsed->expanded = false");
        assert!(state.is_expanded("n"));
        assert!(state.toggle("n"), "toggle again");
        assert!(state.is_collapsed("n"));
    }

    #[test]
    fn collapse_all_nodes() {
        let mut state = ViewCollapseState::new();
        state.collapse_all(&["a", "b", "c"]);
        assert_eq!(state.collapsed_count(), 3);
        assert!(state.is_collapsed("a"));
        assert!(state.is_collapsed("b"));
        assert!(state.is_collapsed("c"));
        assert!(state.is_expanded("d"));
    }

    #[test]
    fn collapse_all_idempotent() {
        let mut state = ViewCollapseState::new();
        state.collapse("a");
        state.collapse_all(&["a", "b"]);
        assert_eq!(state.collapsed_count(), 2);
    }

    #[test]
    fn expand_all() {
        let mut state = ViewCollapseState::new();
        state.collapse_all(&["x", "y", "z"]);
        assert_eq!(state.collapsed_count(), 3);
        state.expand_all();
        assert!(state.all_expanded());
        assert_eq!(state.collapsed_count(), 0);
    }

    #[test]
    fn collapse_ids_sorted() {
        let mut state = ViewCollapseState::new();
        state.collapse("zebra");
        state.collapse("alpha");
        state.collapse("mid");
        let ids = state.collapsed_ids();
        assert_eq!(ids, vec!["alpha", "mid", "zebra"]);
    }

    #[test]
    fn collapse_default() {
        let state = ViewCollapseState::default();
        assert!(state.all_expanded());
    }

    #[test]
    fn collapse_mixed_operations() {
        let mut state = ViewCollapseState::new();
        state.collapse_all(&["a", "b", "c", "d"]);
        state.expand("b");
        state.toggle("c"); // c was collapsed -> now expanded
        state.toggle("e"); // e was expanded -> now collapsed
        assert_eq!(state.collapsed_count(), 3); // a, d, e
        assert!(state.is_collapsed("a"));
        assert!(state.is_expanded("b"));
        assert!(state.is_expanded("c"));
        assert!(state.is_collapsed("d"));
        assert!(state.is_collapsed("e"));
    }



    // -- ext_view extended domain tests ----------------------------------------

    #[test]
    fn y_ext_view_enum_index() {
        assert_eq!(YExtViewExtViewLocation::Sidebar.index(), 0);
        assert_eq!(YExtViewExtViewLocation::Panel.index(), 1);
        assert_eq!(YExtViewExtViewLocation::Editor.index(), 2);
        assert_eq!(YExtViewExtViewLocation::ActivityBar.index(), 3);
    }

    #[test]
    fn y_ext_view_enum_label() {
        assert_eq!(YExtViewExtViewLocation::Sidebar.label(), "Sidebar");
        assert_eq!(YExtViewExtViewLocation::Panel.label(), "Panel");
        assert_eq!(YExtViewExtViewLocation::Editor.label(), "Editor");
        assert_eq!(YExtViewExtViewLocation::ActivityBar.label(), "ActivityBar");
    }

    #[test]
    fn y_ext_view_enum_all() {
        let all = YExtViewExtViewLocation::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_view_enum_is_default() {
        assert!(YExtViewExtViewLocation::Sidebar.is_default());
        assert!(!YExtViewExtViewLocation::ActivityBar.is_default());
    }

    #[test]
    fn y_ext_view_enum_display() {
        assert_eq!(format!("{}", YExtViewExtViewLocation::Sidebar), "Sidebar");
    }

    #[test]
    fn y_ext_view_struct_new() {
        let s = YExtViewExtViewContribution::new();
        let _ = s.summary();
    }

    #[test]
    fn y_ext_view_fingerprint_deterministic() {
        let h1 = y_ext_view_fingerprint("hello");
        let h2 = y_ext_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_view_fingerprint("a"), y_ext_view_fingerprint("b"));
    }

    #[test]
    fn y_ext_view_truncate_short() {
        assert_eq!(y_ext_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_view_truncate_long() {
        let r = y_ext_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_view_normalize_key_basic() {
        assert_eq!(y_ext_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_view_split_path_basic() {
        let parts = y_ext_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_view_count_occurrences_basic() {
        assert_eq!(y_ext_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_view_in_range_basic() {
        assert!(y_ext_view_in_range(5, 1, 10));
        assert!(y_ext_view_in_range(1, 1, 10));
        assert!(y_ext_view_in_range(10, 1, 10));
        assert!(!y_ext_view_in_range(0, 1, 10));
        assert!(!y_ext_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_view_priority_weight() {
        assert_eq!(ZExtViewPriority::Idle.weight(), 0);
        assert_eq!(ZExtViewPriority::Normal.weight(), 2);
        assert_eq!(ZExtViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_view_priority_label() {
        assert_eq!(ZExtViewPriority::Low.label(), "low");
        assert_eq!(ZExtViewPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_view_priority_is_elevated() {
        assert!(!ZExtViewPriority::Normal.is_elevated());
        assert!(ZExtViewPriority::High.is_elevated());
        assert!(ZExtViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_view_priority_display() {
        assert_eq!(format!("{}", ZExtViewPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_view_priority_all_asc() {
        let all = ZExtViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtViewPriority::Idle);
        assert_eq!(all[4], ZExtViewPriority::Realtime);
    }

    #[test]
    fn z_ext_view_struct_new() {
        let s = ZExtViewExtViewBadge::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_view_struct_toggled_clone() {
        let s = ZExtViewExtViewBadge::new();
        let t = s.toggled_clone();
        assert_ne!(s.animate, t.animate);
    }

    #[test]
    fn z_ext_view_rolling_hash_deterministic() {
        let h1 = z_ext_view_rolling_hash(b"test");
        let h2 = z_ext_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_view_rolling_hash(b"a"), z_ext_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_view_pad_to_basic() {
        assert_eq!(z_ext_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_view_is_identifier_basic() {
        assert!(z_ext_view_is_identifier("foo_bar"));
        assert!(z_ext_view_is_identifier("abc123"));
        assert!(!z_ext_view_is_identifier(""));
        assert!(!z_ext_view_is_identifier("has space"));
    }

    #[test]
    fn z_ext_view_levenshtein_basic() {
        assert_eq!(z_ext_view_levenshtein("", ""), 0);
        assert_eq!(z_ext_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_view_unique_words_basic() {
        let w = z_ext_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_view_common_prefix_basic() {
        assert_eq!(z_ext_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_view_struct_clear() {
        let mut s = ZExtViewExtViewBadge::new();
        s.counts.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_view_rolling_hash_empty() {
        let h = z_ext_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 76 ----

    #[test]
    fn xc_76_pool_new_empty() {
        let pool: super::Xc76Pool<i32> = super::Xc76Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_76_pool_release_acquire() {
        let mut pool = super::Xc76Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_76_pool_acquire_empty() {
        let mut pool: super::Xc76Pool<i32> = super::Xc76Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_76_pool_full() {
        let mut pool = super::Xc76Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_76_pool_drain() {
        let mut pool = super::Xc76Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_76_pool_stats() {
        let mut pool = super::Xc76Pool::new(8);
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
    fn xc_76_pool_clear() {
        let mut pool = super::Xc76Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_76_pool_shrink() {
        let mut pool = super::Xc76Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_76_pool_default() {
        let pool: super::Xc76Pool<String> = super::Xc76Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_76_pool_extend() {
        let mut pool = super::Xc76Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_76_pool_retain() {
        let mut pool = super::Xc76Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_76_scheduler_round_robin() {
        let mut sched = super::Xc76Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_76_scheduler_empty() {
        let mut sched = super::Xc76Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_76_scheduler_reset() {
        let mut sched = super::Xc76Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_76_scheduler_add_remove() {
        let mut sched = super::Xc76Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_76_scheduler_targets() {
        let sched = super::Xc76Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_76_hash_empty() {
        assert_eq!(super::xc_76_hash(b""), 5381);
    }

    #[test]
    fn xc_76_hash_data() {
        let h = super::xc_76_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_76_hash(b"hello"), h);
    }

    #[test]
    fn xc_76_reverse_str() {
        assert_eq!(super::xc_76_reverse("abc"), "cba");
        assert_eq!(super::xc_76_reverse(""), "");
    }


    // --- xd_10 deepening tests ---

    #[test]
    fn xd_10_sm_initial_state() {
        let sm = Xd10StateMachine::new();
        assert_eq!(sm.current_state(), Xd10State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_10_sm_valid_idle_to_running() {
        let mut sm = Xd10StateMachine::new();
        assert!(sm.transition(Xd10State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd10State::Running);
    }

    #[test]
    fn xd_10_sm_valid_running_to_paused() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        assert!(sm.transition(Xd10State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd10State::Paused);
    }

    #[test]
    fn xd_10_sm_valid_running_to_done() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        assert!(sm.transition(Xd10State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd10State::Done);
    }

    #[test]
    fn xd_10_sm_valid_paused_to_running() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        sm.transition(Xd10State::Paused).unwrap();
        assert!(sm.transition(Xd10State::Running).is_ok());
    }

    #[test]
    fn xd_10_sm_valid_done_to_idle() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        sm.transition(Xd10State::Done).unwrap();
        assert!(sm.transition(Xd10State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd10State::Idle);
    }

    #[test]
    fn xd_10_sm_invalid_idle_to_done() {
        let mut sm = Xd10StateMachine::new();
        assert!(sm.transition(Xd10State::Done).is_err());
    }

    #[test]
    fn xd_10_sm_invalid_idle_to_paused() {
        let mut sm = Xd10StateMachine::new();
        assert!(sm.transition(Xd10State::Paused).is_err());
    }

    #[test]
    fn xd_10_sm_history_tracking() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        sm.transition(Xd10State::Paused).unwrap();
        sm.transition(Xd10State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd10State::Idle);
        assert_eq!(sm.history()[0].to, Xd10State::Running);
        assert_eq!(sm.history()[1].from, Xd10State::Running);
        assert_eq!(sm.history()[2].to, Xd10State::Done);
    }

    #[test]
    fn xd_10_sm_serialize_deserialize() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd10StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd10State::Running));
    }

    #[test]
    fn xd_10_sm_deserialize_invalid() {
        assert_eq!(Xd10StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_10_sm_reset() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd10State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_10_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd10EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd10Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_10_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd10EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd10Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd10Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_10_bus_unsubscribe() {
        let mut bus = Xd10EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_10_event_kind_and_payload() {
        let e = Xd10Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd10Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_10_bus_clear_history() {
        let mut bus = Xd10EventBus::new();
        bus.publish(Xd10Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_10_sm_step_counter_increments() {
        let mut sm = Xd10StateMachine::new();
        sm.transition(Xd10State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd10State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
