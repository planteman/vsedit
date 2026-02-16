//! View layout management for sidebar, panel, and auxiliary bar.

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
}
