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


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #8
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf8Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf8TrieNode {
    children: std::collections::HashMap<char, Xf8TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf8Trie {
    root: Xf8TrieNode,
    count: usize,
}

impl Xf8Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf8TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf8TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf8TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf8BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf8BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 75).
pub struct Xh75SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh75SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 117 as u64,
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

/// A compact bit set supporting boolean operations (variant 75).
pub struct Xh75BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh75BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 75).
pub struct Xi75Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi75Deque<T> {
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
pub struct Xi75Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi75Interval {
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

/// A simple interval tree (variant 75).
pub struct Xi75IntervalTree {
    xi_intervals: Vec<Xi75Interval>,
}

impl Xi75IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi75Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi75Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi75Interval) -> Vec<&Xi75Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi75Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi75Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi75Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi75Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi75Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi75Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 76) ---

/// Disjoint set / union-find for crate 76.
pub struct Xj76UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj76UnionFind {
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

const XJ76_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 76.
pub struct Xj76BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj76BTreeNode<K, V>>>,
    len: usize,
}

struct Xj76BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj76BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj76BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ76_BTREE_ORDER - 1
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
        let mid = XJ76_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj76BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj76BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj76BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj76BTreeNode::xj_new_leaf();
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


// --- xk_75 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk75SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk75SegmentTree {
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
pub struct Xk75DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk75DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_76).
#[derive(Debug, Clone)]
pub struct Xl76Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl76Rope {
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

/// Suffix array for efficient string searching (xl_76).
#[derive(Debug, Clone)]
pub struct Xl76SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl76SuffixArray {
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
pub struct Xm76MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm76MatrixSparse {
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
pub struct Xm76Tokenizer {
    text: String,
}

impl Xm76Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 75.
pub struct Xn75Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn75Fenwick {
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

// ----- AVL tree map — crate 75 -----

#[derive(Debug, Clone)]
struct Xn75AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn75AvlNode<K, V>>>,
    right: Option<Box<Xn75AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 75.
#[derive(Debug, Clone)]
pub struct Xn75AVL<K, V> {
    root: Option<Box<Xn75AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn75AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn75AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn75AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn75AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn75AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn75AvlNode<K, V>>) -> Box<Xn75AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn75AvlNode<K, V>>) -> Box<Xn75AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn75AvlNode<K, V>>) -> Box<Xn75AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn75AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn75AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn75AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn75AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn75AvlNode<K, V>>) -> &Xn75AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn75AvlNode<K, V>>) -> (Box<Xn75AvlNode<K, V>>, Option<Box<Xn75AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn75AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn75AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn75AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn75AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn75AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn75AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn75AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo75RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo75Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo75RBNode<K, V> {
    key: K,
    value: V,
    color: Xo75Color,
    left: Option<Box<Xo75RBNode<K, V>>>,
    right: Option<Box<Xo75RBNode<K, V>>>,
}

/// A red-black tree map for crate 75.
#[derive(Debug, Clone)]
pub struct Xo75RedBlack<K, V> {
    root: Option<Box<Xo75RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo75RedBlack<K, V> {
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
            r.color = Xo75Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo75RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo75RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo75RBNode {
                    key, value, color: Xo75Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo75RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo75Color::Red)
    }

    fn xo_balance(mut h: Box<Xo75RBNode<K, V>>) -> Box<Xo75RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo75Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo75RBNode<K, V>>) -> Box<Xo75RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo75Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo75RBNode<K, V>>) -> Box<Xo75RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo75Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo75RBNode<K, V>>) {
        h.color = Xo75Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo75Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo75Color::Black; }
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
            r.color = Xo75Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo75RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo75RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo75RBNode<K, V>) -> (K, V, Option<Box<Xo75RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo75RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo75Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo75RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo75ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 75.
#[derive(Debug, Clone)]
pub struct Xo75ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo75ConsistentHash {
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
            let vkey = format!("{}#xo75#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo75#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 75).
#[derive(Debug)]
pub struct Xp75SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp75Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp75Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp75Node<K, V>>>,
    xp_right: Option<Box<Xp75Node<K, V>>>,
}

impl<K: Ord, V> Xp75Node<K, V> {
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

impl<K: Ord, V> Default for Xp75SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp75SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp75Node<K, V>>>, key: &K) -> Option<Box<Xp75Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp75Node<K, V>>) -> Box<Xp75Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp75Node<K, V>>) -> Box<Xp75Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp75Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp75Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp75Node::xp_new(key, val));
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


// --------------- Xq75Treap ---------------

use std::cmp::Ordering as Xq75Ord;

struct Xq75TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq75TreapNode<K, V>>>,
    right: Option<Box<Xq75TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq75Treap<K, V> {
    root: Option<Box<Xq75TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq75TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_75_size<K, V>(node: &Option<Box<Xq75TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_75_update_size<K, V>(node: &mut Xq75TreapNode<K, V>) {
    node.size = 1 + xq_75_size(&node.left) + xq_75_size(&node.right);
}

fn xq_75_rotate_right<K, V>(mut node: Box<Xq75TreapNode<K, V>>) -> Box<Xq75TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_75_update_size(&mut node);
    left.right = Some(node);
    xq_75_update_size(&mut left);
    left
}

fn xq_75_rotate_left<K, V>(mut node: Box<Xq75TreapNode<K, V>>) -> Box<Xq75TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_75_update_size(&mut node);
    right.left = Some(node);
    xq_75_update_size(&mut right);
    right
}

fn xq_75_insert_node<K: Ord, V>(
    node: Option<Box<Xq75TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq75TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq75TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq75Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq75Ord::Less => {
                let (new_left, old) = xq_75_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_75_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_75_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq75Ord::Greater => {
                let (new_right, old) = xq_75_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_75_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_75_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_75_remove_node<K: Ord, V>(
    node: Option<Box<Xq75TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq75TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq75Ord::Less => {
                let (new_left, old) = xq_75_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_75_update_size(&mut n);
                (Some(n), old)
            }
            Xq75Ord::Greater => {
                let (new_right, old) = xq_75_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_75_update_size(&mut n);
                (Some(n), old)
            }
            Xq75Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_75_rotate_right(n);
                    let (new_right, old) = xq_75_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_75_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_75_rotate_left(n);
                    let (new_left, old) = xq_75_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_75_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_75_find_min<K, V>(node: &Option<Box<Xq75TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_75_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_75_find_max<K, V>(node: &Option<Box<Xq75TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_75_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_75_rank<K: Ord, V>(node: &Option<Box<Xq75TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq75Ord::Less => xq_75_rank(&n.left, key),
            Xq75Ord::Equal => xq_75_size(&n.left),
            Xq75Ord::Greater => 1 + xq_75_size(&n.left) + xq_75_rank(&n.right, key),
        },
    }
}

fn xq_75_kth<K, V>(node: &Option<Box<Xq75TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_75_size(&n.left);
        if k < left_size {
            xq_75_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_75_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_75_in_order<K: Clone, V>(node: &Option<Box<Xq75TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_75_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_75_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq75Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 75 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_75_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq75Ord::Equal => return Some(&n.value),
                Xq75Ord::Less => cur = &n.left,
                Xq75Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_75_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_75_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_75_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_75_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_75_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_75_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_75_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq75VEBTree ---------------

pub struct Xq75VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq75VEBTree>>,
    clusters: Vec<Option<Box<Xq75VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq75VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq75VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq75VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr75KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr75KDPoint {
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
pub struct Xr75BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr75KDNode {
    xr_point: Xr75KDPoint,
    xr_left: Option<Box<Xr75KDNode>>,
    xr_right: Option<Box<Xr75KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr75KDTree {
    xr_root: Option<Box<Xr75KDNode>>,
    xr_size: usize,
}

impl Xr75KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr75KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr75KDNode>>,
        point: Xr75KDPoint,
        depth: usize,
    ) -> Box<Xr75KDNode> {
        match node {
            None => Box::new(Xr75KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr75KDPoint) -> Option<Xr75KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr75KDNode>,
        query: &Xr75KDPoint,
        depth: usize,
        best: &mut Xr75KDPoint,
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
    ) -> Vec<Xr75KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr75KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr75KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr75KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr75KDNode>>, pts: &mut Vec<Xr75KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr75KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr75BoundingBox> {
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
        Some(Xr75BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs76PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs76PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs76PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs76PersistentArray {
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
pub struct Xs76ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs76ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs76ConcurrentQueue {
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
pub struct Xs76RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs76RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs76RangeMap {
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
pub struct Xs76CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs76CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs76CircularBuffer {
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

/// Auxiliary statistics tracker for xs_75 data structures.
#[derive(Debug, Clone)]
pub struct Xs75StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs75StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs75StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
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


    // -- xf_ trie + bloom tests for instance #8 --

    #[test]
    fn xf8_trie_insert_search() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf8_trie_starts_with() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf8_trie_remove() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf8_trie_word_count() {
        let mut t = Xf8Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf8_trie_longest_prefix() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf8_trie_all_words() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf8_trie_autocomplete() {
        let mut t = Xf8Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf8_trie_empty_search() {
        let t = Xf8Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf8_bloom_add_contains() {
        let mut bf = Xf8BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf8_bloom_probably_absent() {
        let bf = Xf8BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf8_bloom_false_positive_rate() {
        let mut bf = Xf8BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf8_bloom_clear() {
        let mut bf = Xf8BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf8_bloom_union() {
        let mut a = Xf8BloomFilter::xf_new(512, 2);
        let mut b = Xf8BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf8_bloom_intersection_estimate() {
        let mut a = Xf8BloomFilter::xf_new(512, 2);
        let mut b = Xf8BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf8_bloom_union_size_mismatch() {
        let a = Xf8BloomFilter::xf_new(256, 2);
        let b = Xf8BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh75_skip_insert_contains() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh75_skip_remove() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh75_skip_len() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh75_skip_range_query() {
        let mut sl = super::Xh75SkipList::xh_new(4);
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
    fn xh75_skip_floor_ceiling() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh75_skip_rank() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh75_skip_empty() {
        let sl = super::Xh75SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh75_skip_duplicates() {
        let mut sl = super::Xh75SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh75_bitset_set_test() {
        let mut bs = super::Xh75BitSet::xh_new(256);
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
    fn xh75_bitset_clear_count() {
        let mut bs = super::Xh75BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh75_bitset_and_or_xor() {
        let mut a = super::Xh75BitSet::xh_new(128);
        let mut b = super::Xh75BitSet::xh_new(128);
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
    fn xh75_bitset_iter_ones() {
        let mut bs = super::Xh75BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh75_bitset_first_last() {
        let mut bs = super::Xh75BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh75_bitset_empty() {
        let bs = super::Xh75BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi75_deque_push_pop_back() {
        let mut dq = super::Xi75Deque::xi_new(4);
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
    fn xi75_deque_push_pop_front() {
        let mut dq = super::Xi75Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi75_deque_mixed_ops() {
        let mut dq = super::Xi75Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi75_deque_get_and_split() {
        let mut dq = super::Xi75Deque::xi_new(8);
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
    fn xi75_deque_rotate_left() {
        let mut dq = super::Xi75Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi75_deque_rotate_right() {
        let mut dq = super::Xi75Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi75_deque_grow() {
        let mut dq = super::Xi75Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi75_deque_empty() {
        let dq = super::Xi75Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi75_interval_tree_insert_query() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi75Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi75Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi75_interval_tree_overlap() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi75Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi75Interval::xi_new(12, 20));
        let q = super::Xi75Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi75_interval_tree_remove() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi75Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi75_interval_tree_gaps() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi75Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi75Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi75Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi75Interval::xi_new(8, 10));
    }

    #[test]
    fn xi75_interval_tree_merge() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi75Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi75Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi75Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi75Interval::xi_new(10, 15));
    }

    #[test]
    fn xi75_interval_tree_all() {
        let mut tree = super::Xi75IntervalTree::xi_new();
        tree.xi_insert(super::Xi75Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi75Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi75_interval_tree_empty() {
        let tree = super::Xi75IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi75_interval_tree_contains_point() {
        let iv = super::Xi75Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 76) ---

    #[test]
    fn xj_76_uf_make_and_find() {
        let mut uf = super::Xj76UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_76_uf_union_connected() {
        let mut uf = super::Xj76UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_76_uf_component_count() {
        let mut uf = super::Xj76UnionFind::xj_new();
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
    fn xj_76_uf_component_size() {
        let mut uf = super::Xj76UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_76_uf_largest_component() {
        let mut uf = super::Xj76UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_76_uf_many_elements() {
        let mut uf = super::Xj76UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_76_uf_separate_components() {
        let mut uf = super::Xj76UnionFind::xj_new();
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
    fn xj_76_uf_path_compression() {
        let mut uf = super::Xj76UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_76_bt_insert_get() {
        let mut bt = super::Xj76BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_76_bt_contains_len() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_76_bt_replace() {
        let mut bt = super::Xj76BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_76_bt_remove() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_76_bt_keys_values() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_76_bt_range() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_76_bt_min_max() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_76_bt_many_inserts() {
        let mut bt = super::Xj76BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_75 segment tree tests ---

    #[test]
    fn xk_75_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_75_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk75SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_75_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_75_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_75_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_75_st_single_element() {
        let data = vec![42];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_75_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk75SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_75_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk75SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_75 disjoint intervals tests ---

    #[test]
    fn xk_75_di_add_and_count() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_75_di_merge_overlap() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_75_di_contains() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_75_di_remove() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_75_di_covered_length() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_75_di_gaps() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_75_di_merge_adjacent() {
        let mut di = super::Xk75DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_75_di_empty() {
        let di = super::Xk75DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_76_rope_new_empty() {
        let rope = super::Xl76Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_76_rope_from_str() {
        let rope = super::Xl76Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_76_rope_insert_at() {
        let mut rope = super::Xl76Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_76_rope_delete_range() {
        let mut rope = super::Xl76Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_76_rope_char_at() {
        let rope = super::Xl76Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_76_rope_split_concat() {
        let rope = super::Xl76Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_76_rope_line_count() {
        let rope = super::Xl76Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_76_rope_line_at() {
        let rope = super::Xl76Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_76_sa_build_and_search() {
        let sa = super::Xl76SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_76_sa_count() {
        let sa = super::Xl76SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_76_sa_longest_repeated() {
        let sa = super::Xl76SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_76_sa_all_positions() {
        let sa = super::Xl76SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_76_sa_len() {
        let sa = super::Xl76SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_76_sa_empty() {
        let sa = super::Xl76SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_76_rope_slice() {
        let rope = super::Xl76Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_76_sa_search_start() {
        let sa = super::Xl76SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_76_sparse_set_get() {
        let mut m = super::Xm76MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_76_sparse_row_col() {
        let mut m = super::Xm76MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_76_sparse_transpose() {
        let mut m = super::Xm76MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_76_sparse_multiply_vec() {
        let mut m = super::Xm76MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_76_sparse_nnz_density() {
        let mut m = super::Xm76MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_76_sparse_clear() {
        let mut m = super::Xm76MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_76_sparse_overwrite_zero() {
        let mut m = super::Xm76MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_76_tokenizer_basic() {
        let t = super::Xm76Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_76_tokenizer_count() {
        let t = super::Xm76Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_76_tokenizer_unique() {
        let t = super::Xm76Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_76_tokenizer_frequency() {
        let t = super::Xm76Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_76_tokenizer_delimiter() {
        let t = super::Xm76Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_76_tokenizer_whitespace() {
        let t = super::Xm76Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_76_tokenizer_empty() {
        let t = super::Xm76Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 75 ----

    #[test]
    fn xn_75_fenwick_prefix_sum() {
        let mut ft = super::Xn75Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_75_fenwick_range_sum() {
        let mut ft = super::Xn75Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_75_fenwick_point_query() {
        let mut ft = super::Xn75Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_75_fenwick_len() {
        let ft = super::Xn75Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_75_fenwick_multiple_updates() {
        let mut ft = super::Xn75Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_75_fenwick_single_element() {
        let mut ft = super::Xn75Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_75_fenwick_find_kth() {
        let mut ft = super::Xn75Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_75_fenwick_negative_delta() {
        let mut ft = super::Xn75Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 75 ----

    #[test]
    fn xn_75_avl_insert_get() {
        let mut m = super::Xn75AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_75_avl_remove() {
        let mut m = super::Xn75AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_75_avl_in_order() {
        let mut m = super::Xn75AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_75_avl_min_max() {
        let mut m = super::Xn75AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_75_avl_floor_ceiling() {
        let mut m = super::Xn75AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_75_avl_height_balanced() {
        let mut m = super::Xn75AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_75_avl_overwrite() {
        let mut m = super::Xn75AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_75_avl_empty() {
        let m: super::Xn75AVL<i32, i32> = super::Xn75AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo75RedBlack tests ---

    #[test]
    fn xo_75_rb_insert_and_get() {
        let mut tree = super::Xo75RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_75_rb_len_and_empty() {
        let mut tree = super::Xo75RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_75_rb_min_max() {
        let mut tree = super::Xo75RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_75_rb_contains() {
        let mut tree = super::Xo75RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_75_rb_remove() {
        let mut tree = super::Xo75RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_75_rb_in_order() {
        let mut tree = super::Xo75RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_75_rb_black_height() {
        let mut tree = super::Xo75RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_75_rb_overwrite() {
        let mut tree = super::Xo75RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo75ConsistentHash tests ---

    #[test]
    fn xo_75_ch_add_and_count() {
        let mut ring = super::Xo75ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_75_ch_remove_node() {
        let mut ring = super::Xo75ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_75_ch_get_node() {
        let mut ring = super::Xo75ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_75_ch_empty_ring() {
        let ring = super::Xo75ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_75_ch_distribution() {
        let mut ring = super::Xo75ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_75_ch_rebalance() {
        let mut ring = super::Xo75ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_75_ch_virtual_nodes() {
        let mut ring = super::Xo75ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_75_ch_consistent_lookup() {
        let mut ring = super::Xo75ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_75_splay_insert_get() {
        let mut t = super::Xp75SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_75_splay_remove() {
        let mut t = super::Xp75SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_75_splay_count_increases() {
        let mut t = super::Xp75SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_75_splay_depth() {
        let mut t = super::Xp75SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_75_splay_len_empty() {
        let t = super::Xp75SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_75_splay_min_max() {
        let mut t = super::Xp75SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_75_splay_overwrite() {
        let mut t = super::Xp75SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_75_splay_remove_missing() {
        let mut t = super::Xp75SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_75 treap tests ----
    #[test]
    fn xq_75_treap_empty() {
        let t = super::Xq75Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_75_treap_insert_get() {
        let mut t = super::Xq75Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_75_treap_overwrite() {
        let mut t = super::Xq75Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_75_treap_remove() {
        let mut t = super::Xq75Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_75_treap_min_max() {
        let mut t = super::Xq75Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_75_treap_rank() {
        let mut t = super::Xq75Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_75_treap_kth() {
        let mut t = super::Xq75Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_75_treap_in_order() {
        let mut t = super::Xq75Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_75 VEB tree tests ----
    #[test]
    fn xq_75_veb_empty() {
        let v = super::Xq75VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_75_veb_insert_contains() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_75_veb_min_max() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_75_veb_delete() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_75_veb_successor() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_75_veb_predecessor() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_75_veb_count() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_75_veb_duplicate_insert() {
        let mut v = super::Xq75VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_75_kdtree_empty() {
        let tree = super::Xr75KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_75_kdtree_insert_one() {
        let mut tree = super::Xr75KDTree::xr_new();
        tree.xr_insert(super::Xr75KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_75_kdtree_insert_multiple() {
        let mut tree = super::Xr75KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr75KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_75_kdtree_nearest_neighbor() {
        let mut tree = super::Xr75KDTree::xr_new();
        tree.xr_insert(super::Xr75KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr75KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr75KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_75_kdtree_nn_empty() {
        let tree = super::Xr75KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr75KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_75_kdtree_range_search() {
        let mut tree = super::Xr75KDTree::xr_new();
        tree.xr_insert(super::Xr75KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr75KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr75KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_75_kdtree_range_empty() {
        let mut tree = super::Xr75KDTree::xr_new();
        tree.xr_insert(super::Xr75KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_75_kdtree_all_points() {
        let mut tree = super::Xr75KDTree::xr_new();
        tree.xr_insert(super::Xr75KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr75KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_75_kdtree_depth() {
        let mut tree = super::Xr75KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr75KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_75_kdtree_bounding_box() {
        let mut tree = super::Xr75KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr75KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr75KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_76_persistent_array_new() {
        let arr = super::Xs76PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_76_persistent_array_push() {
        let mut arr = super::Xs76PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_76_persistent_array_set() {
        let mut arr = super::Xs76PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_76_persistent_array_diff() {
        let mut arr = super::Xs76PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_76_persistent_array_rollback() {
        let mut arr = super::Xs76PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_76_persistent_array_history() {
        let mut arr = super::Xs76PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_76_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs76PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_76_persistent_array_from_vec() {
        let arr = super::Xs76PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_76_concurrent_queue_new() {
        let q = super::Xs76ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_76_concurrent_queue_push_pop() {
        let mut q = super::Xs76ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_76_concurrent_queue_full() {
        let mut q = super::Xs76ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_76_concurrent_queue_drain() {
        let mut q = super::Xs76ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_76_concurrent_queue_try_pop() {
        let mut q = super::Xs76ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_76_concurrent_queue_clear() {
        let mut q = super::Xs76ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_76_range_map_new() {
        let rm = super::Xs76RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_76_range_map_insert_get() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_76_range_map_overlap() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_76_range_map_remove() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_76_range_map_gaps() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_76_range_map_coverage() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_76_range_map_contains() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_76_range_map_clear() {
        let mut rm = super::Xs76RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_76_circular_buffer_new() {
        let buf = super::Xs76CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_76_circular_buffer_push_pop() {
        let mut buf = super::Xs76CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_76_circular_buffer_overwrite() {
        let mut buf = super::Xs76CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_76_circular_buffer_peek() {
        let mut buf = super::Xs76CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_76_circular_buffer_is_full() {
        let mut buf = super::Xs76CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_76_circular_buffer_iter() {
        let mut buf = super::Xs76CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_76_circular_buffer_clear() {
        let mut buf = super::Xs76CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_76_circular_buffer_to_vec() {
        let mut buf = super::Xs76CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_75_stats_tracker_new() {
        let tracker = super::Xs75StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_75_stats_tracker_mean() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_75_stats_tracker_min_max() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_75_stats_tracker_median() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_75_stats_tracker_variance() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_75_stats_tracker_range() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_75_stats_tracker_clear() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_75_stats_tracker_sum() {
        let mut tracker = super::Xs75StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
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

}
