//! Extensions marketplace view.
//!
//! RPC bridge between the extension host and the main thread for custom views.

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
}
