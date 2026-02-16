//! View layout management for sidebar, panel, and auxiliary bar.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewContainerLocation {
    Sidebar,
    Panel,
    AuxiliaryBar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDescriptor {
    pub id: String,
    pub name: String,
    pub container: ViewContainerLocation,
    pub order: i32,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Bottom,
    Left,
    Right,
}

pub struct ViewLayout {
    pub views: Vec<ViewDescriptor>,
    pub panel_position: PanelPosition,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub auxiliary_bar_visible: bool,
}

impl ViewLayout {
    pub fn new() -> Self {
        Self {
            views: Vec::new(),
            panel_position: PanelPosition::Bottom,
            sidebar_visible: true,
            panel_visible: true,
            auxiliary_bar_visible: false,
        }
    }

    pub fn add_view(&mut self, desc: ViewDescriptor) {
        self.views.push(desc);
    }

    pub fn remove_view(&mut self, id: &str) -> bool {
        let len = self.views.len();
        self.views.retain(|v| v.id != id);
        self.views.len() < len
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn set_panel_position(&mut self, pos: PanelPosition) {
        self.panel_position = pos;
    }

    pub fn get_views_in(&self, container: ViewContainerLocation) -> Vec<&ViewDescriptor> {
        self.views.iter().filter(|v| v.container == container).collect()
    }

    pub fn set_view_visibility(&mut self, id: &str, visible: bool) {
        if let Some(v) = self.views.iter_mut().find(|v| v.id == id) {
            v.visible = visible;
        }
    }
}

impl Default for ViewLayout {
    fn default() -> Self {
        Self::new()
    }
}

// --- Display impls ---

impl fmt::Display for ViewContainerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sidebar => write!(f, "Sidebar"),
            Self::Panel => write!(f, "Panel"),
            Self::AuxiliaryBar => write!(f, "AuxiliaryBar"),
        }
    }
}

impl fmt::Display for PanelPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bottom => write!(f, "Bottom"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

impl fmt::Display for ViewLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = |b: bool| if b { "visible" } else { "hidden" };
        write!(
            f,
            "Layout: {} views, sidebar={}, panel={}",
            self.views.len(),
            vis(self.sidebar_visible),
            vis(self.panel_visible),
        )
    }
}

// --- Error type ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    ViewNotFound(String),
    DuplicateView(String),
    InvalidPosition(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ViewNotFound(id) => write!(f, "view not found: {id}"),
            Self::DuplicateView(id) => write!(f, "duplicate view: {id}"),
            Self::InvalidPosition(msg) => write!(f, "invalid position: {msg}"),
        }
    }
}

impl std::error::Error for LayoutError {}

// --- Additional ViewLayout methods ---

impl ViewLayout {
    /// Like `add_view` but returns an error on duplicate id.
    pub fn try_add_view(&mut self, desc: ViewDescriptor) -> Result<(), LayoutError> {
        if self.views.iter().any(|v| v.id == desc.id) {
            return Err(LayoutError::DuplicateView(desc.id));
        }
        self.views.push(desc);
        Ok(())
    }

    /// Move a view to a different container.
    pub fn move_view(
        &mut self,
        id: &str,
        target: ViewContainerLocation,
    ) -> Result<(), LayoutError> {
        let view = self
            .views
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| LayoutError::ViewNotFound(id.to_string()))?;
        view.container = target;
        Ok(())
    }

    /// Sort views by `order` within each container, preserving relative order of
    /// equal elements.
    pub fn sort_views(&mut self) {
        self.views.sort_by_key(|v| (v.container as u8, v.order));
    }

    /// Look up a view by id.
    pub fn get_view(&self, id: &str) -> Option<&ViewDescriptor> {
        self.views.iter().find(|v| v.id == id)
    }

    /// Count of views that are currently visible.
    pub fn visible_view_count(&self) -> usize {
        self.views.iter().filter(|v| v.visible).count()
    }

    /// Toggle the auxiliary bar visibility.
    pub fn toggle_auxiliary_bar(&mut self) {
        self.auxiliary_bar_visible = !self.auxiliary_bar_visible;
    }
}

// --- Builder ---

/// Builder for constructing a `ViewDescriptor` step-by-step.
#[derive(Debug, Clone)]
pub struct ViewDescriptorBuilder {
    id: String,
    name: Option<String>,
    container: ViewContainerLocation,
    order: i32,
    visible: bool,
}

impl ViewDescriptorBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            container: ViewContainerLocation::Sidebar,
            order: 0,
            visible: true,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn container(mut self, container: ViewContainerLocation) -> Self {
        self.container = container;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn build(self) -> ViewDescriptor {
        ViewDescriptor {
            name: self.name.unwrap_or_else(|| self.id.clone()),
            id: self.id,
            container: self.container,
            order: self.order,
            visible: self.visible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_view(id: &str, container: ViewContainerLocation) -> ViewDescriptor {
        ViewDescriptor {
            id: id.to_string(),
            name: id.to_string(),
            container,
            order: 0,
            visible: true,
        }
    }

    #[test]
    fn add_and_remove_views() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("terminal", ViewContainerLocation::Panel));
        assert_eq!(layout.views.len(), 2);
        assert!(layout.remove_view("explorer"));
        assert!(!layout.remove_view("explorer"));
        assert_eq!(layout.views.len(), 1);
    }

    #[test]
    fn toggle_sidebar_and_panel() {
        let mut layout = ViewLayout::new();
        assert!(layout.sidebar_visible);
        layout.toggle_sidebar();
        assert!(!layout.sidebar_visible);
        layout.toggle_panel();
        assert!(!layout.panel_visible);
    }

    #[test]
    fn get_views_in_container() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("search", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("terminal", ViewContainerLocation::Panel));
        let sidebar = layout.get_views_in(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar.len(), 2);
        let panel = layout.get_views_in(ViewContainerLocation::Panel);
        assert_eq!(panel.len(), 1);
    }

    #[test]
    fn set_view_visibility() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.set_view_visibility("explorer", false);
        assert!(!layout.views[0].visible);
    }

    #[test]
    fn try_add_view_duplicate() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        let res = layout.try_add_view(sample_view("explorer", ViewContainerLocation::Panel));
        assert_eq!(res, Err(LayoutError::DuplicateView("explorer".into())));
        assert_eq!(layout.views.len(), 1);
    }

    #[test]
    fn try_add_view_ok() {
        let mut layout = ViewLayout::new();
        assert!(layout.try_add_view(sample_view("a", ViewContainerLocation::Sidebar)).is_ok());
        assert!(layout.try_add_view(sample_view("b", ViewContainerLocation::Panel)).is_ok());
        assert_eq!(layout.views.len(), 2);
    }

    #[test]
    fn move_view_between_containers() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        assert!(layout.move_view("explorer", ViewContainerLocation::Panel).is_ok());
        assert_eq!(layout.views[0].container, ViewContainerLocation::Panel);
    }

    #[test]
    fn move_view_not_found() {
        let mut layout = ViewLayout::new();
        let res = layout.move_view("missing", ViewContainerLocation::Panel);
        assert_eq!(res, Err(LayoutError::ViewNotFound("missing".into())));
    }

    #[test]
    fn sort_views_by_order() {
        let mut layout = ViewLayout::new();
        let mut v1 = sample_view("b", ViewContainerLocation::Sidebar);
        v1.order = 2;
        let mut v2 = sample_view("a", ViewContainerLocation::Sidebar);
        v2.order = 1;
        layout.add_view(v1);
        layout.add_view(v2);
        layout.add_view(sample_view("c", ViewContainerLocation::Panel));
        layout.sort_views();
        assert_eq!(layout.views[0].id, "a");
        assert_eq!(layout.views[1].id, "b");
        assert_eq!(layout.views[2].id, "c");
    }

    #[test]
    fn get_view_by_id() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        assert!(layout.get_view("explorer").is_some());
        assert!(layout.get_view("nope").is_none());
    }

    #[test]
    fn visible_view_count() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("b", ViewContainerLocation::Sidebar));
        layout.set_view_visibility("b", false);
        assert_eq!(layout.visible_view_count(), 1);
    }

    #[test]
    fn toggle_auxiliary_bar() {
        let mut layout = ViewLayout::new();
        assert!(!layout.auxiliary_bar_visible);
        layout.toggle_auxiliary_bar();
        assert!(layout.auxiliary_bar_visible);
        layout.toggle_auxiliary_bar();
        assert!(!layout.auxiliary_bar_visible);
    }

    #[test]
    fn display_impls() {
        assert_eq!(ViewContainerLocation::Sidebar.to_string(), "Sidebar");
        assert_eq!(ViewContainerLocation::Panel.to_string(), "Panel");
        assert_eq!(ViewContainerLocation::AuxiliaryBar.to_string(), "AuxiliaryBar");
        assert_eq!(PanelPosition::Bottom.to_string(), "Bottom");
        assert_eq!(PanelPosition::Left.to_string(), "Left");
        assert_eq!(PanelPosition::Right.to_string(), "Right");
    }

    #[test]
    fn display_view_layout() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        let s = layout.to_string();
        assert!(s.contains("1 views"));
        assert!(s.contains("sidebar=visible"));
        assert!(s.contains("panel=visible"));
    }

    #[test]
    fn builder_defaults() {
        let desc = ViewDescriptorBuilder::new("explorer").build();
        assert_eq!(desc.id, "explorer");
        assert_eq!(desc.name, "explorer");
        assert_eq!(desc.container, ViewContainerLocation::Sidebar);
        assert_eq!(desc.order, 0);
        assert!(desc.visible);
    }

    #[test]
    fn builder_full() {
        let desc = ViewDescriptorBuilder::new("term")
            .name("Terminal")
            .container(ViewContainerLocation::Panel)
            .order(5)
            .visible(false)
            .build();
        assert_eq!(desc.id, "term");
        assert_eq!(desc.name, "Terminal");
        assert_eq!(desc.container, ViewContainerLocation::Panel);
        assert_eq!(desc.order, 5);
        assert!(!desc.visible);
    }

    #[test]
    fn error_display() {
        let e1 = LayoutError::ViewNotFound("x".into());
        assert_eq!(e1.to_string(), "view not found: x");
        let e2 = LayoutError::DuplicateView("y".into());
        assert_eq!(e2.to_string(), "duplicate view: y");
        let e3 = LayoutError::InvalidPosition("bad".into());
        assert_eq!(e3.to_string(), "invalid position: bad");
    }
}
