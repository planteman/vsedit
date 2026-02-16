//! View container system — manages view containers and views.
//!
//! This crate provides the view registration service equivalent to VS Code's
//! views system, including [`ViewContainer`], [`ViewDescriptor`], and the
//! central [`ViewsRegistry`].

use std::collections::HashMap;
use std::fmt;

use vsedit_contextkey::ContextKeyExpr;
use vsedit_events::{Emitter, Event};

// ---------------------------------------------------------------------------
// ViewContainerLocation
// ---------------------------------------------------------------------------

/// Where a view container is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewContainerLocation {
    Sidebar,
    Panel,
    AuxiliaryBar,
}

// ---------------------------------------------------------------------------
// ViewContainer
// ---------------------------------------------------------------------------

/// A container that holds views (e.g., Explorer, Source Control).
#[derive(Debug, Clone)]
pub struct ViewContainer {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub location: ViewContainerLocation,
    pub order: i32,
}

// ---------------------------------------------------------------------------
// ViewDescriptor
// ---------------------------------------------------------------------------

/// A registered view within a container.
#[derive(Debug, Clone)]
pub struct ViewDescriptor {
    pub id: String,
    pub name: String,
    pub container_id: String,
    pub when: Option<ContextKeyExpr>,
    pub order: i32,
    pub can_toggle_visibility: bool,
}

// ---------------------------------------------------------------------------
// ViewsRegistry
// ---------------------------------------------------------------------------

/// Central registry for view containers and views.
pub struct ViewsRegistry {
    containers: Vec<ViewContainer>,
    views: Vec<ViewDescriptor>,
    active_container: HashMap<ViewContainerLocation, String>,
    on_did_change: Emitter<()>,
}

impl ViewsRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            views: Vec::new(),
            active_container: HashMap::new(),
            on_did_change: Emitter::new(),
        }
    }

    /// Register a view container.
    pub fn register_container(&mut self, container: ViewContainer) {
        self.containers.push(container);
        self.on_did_change.fire(&());
    }

    /// Register a view descriptor.
    pub fn register_view(&mut self, view: ViewDescriptor) {
        self.views.push(view);
        self.on_did_change.fire(&());
    }

    /// Get all containers at a given location, sorted by order.
    pub fn get_containers(&self, location: ViewContainerLocation) -> Vec<&ViewContainer> {
        let mut result: Vec<&ViewContainer> = self
            .containers
            .iter()
            .filter(|c| c.location == location)
            .collect();
        result.sort_by_key(|c| c.order);
        result
    }

    /// Get all views belonging to a container, sorted by order.
    pub fn get_views(&self, container_id: &str) -> Vec<&ViewDescriptor> {
        let mut result: Vec<&ViewDescriptor> = self
            .views
            .iter()
            .filter(|v| v.container_id == container_id)
            .collect();
        result.sort_by_key(|v| v.order);
        result
    }

    /// Set the active container for a location.
    pub fn set_active_container(&mut self, location: ViewContainerLocation, id: &str) {
        self.active_container.insert(location, id.to_string());
        self.on_did_change.fire(&());
    }

    /// Get the active container id for a location.
    pub fn get_active_container(&self, location: ViewContainerLocation) -> Option<&str> {
        self.active_container.get(&location).map(|s| s.as_str())
    }

    /// Subscribe to registry change events.
    pub fn on_did_change(&self) -> Event<()> {
        self.on_did_change.event()
    }
}

impl Default for ViewsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl fmt::Display for ViewContainerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewContainerLocation::Sidebar => write!(f, "Sidebar"),
            ViewContainerLocation::Panel => write!(f, "Panel"),
            ViewContainerLocation::AuxiliaryBar => write!(f, "AuxiliaryBar"),
        }
    }
}

impl fmt::Display for ViewContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.title, self.location)
    }
}

impl fmt::Display for ViewDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (in {})", self.name, self.container_id)
    }
}

// ---------------------------------------------------------------------------
// ViewVisibility
// ---------------------------------------------------------------------------

/// Tracks which views are visible or hidden.
#[derive(Debug, Clone)]
pub struct ViewVisibility {
    visibility: HashMap<String, bool>,
}

impl ViewVisibility {
    /// Create a new `ViewVisibility` with no overrides.
    pub fn new() -> Self {
        Self {
            visibility: HashMap::new(),
        }
    }

    /// Set the visibility of a view.
    pub fn set_visible(&mut self, view_id: &str, visible: bool) {
        self.visibility.insert(view_id.to_string(), visible);
    }

    /// Check whether a view is visible. Defaults to `true` if not explicitly set.
    pub fn is_visible(&self, view_id: &str) -> bool {
        self.visibility.get(view_id).copied().unwrap_or(true)
    }

    /// Return the ids of all explicitly hidden views.
    pub fn hidden_views(&self) -> Vec<&str> {
        self.visibility
            .iter()
            .filter(|&(_, &v)| !v)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Count of entries that are explicitly set to visible.
    pub fn visible_count(&self) -> usize {
        self.visibility.values().filter(|&&v| v).count()
    }

    /// Toggle the visibility of a view. If not yet tracked, the default
    /// (`true`) is toggled to `false`.
    pub fn toggle(&mut self, view_id: &str) {
        let current = self.is_visible(view_id);
        self.set_visible(view_id, !current);
    }

    /// Clear all visibility overrides.
    pub fn reset(&mut self) {
        self.visibility.clear();
    }
}

impl Default for ViewVisibility {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Extra ViewsRegistry helpers
// ---------------------------------------------------------------------------

impl ViewsRegistry {
    /// Total number of registered containers.
    pub fn container_count(&self) -> usize {
        self.containers.len()
    }

    /// Total number of registered views.
    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// Find a container by its id.
    pub fn get_container(&self, id: &str) -> Option<&ViewContainer> {
        self.containers.iter().find(|c| c.id == id)
    }

    /// Find a view by its id.
    pub fn get_view(&self, id: &str) -> Option<&ViewDescriptor> {
        self.views.iter().find(|v| v.id == id)
    }

    /// Remove a container by id. Returns `true` if it was found and removed.
    pub fn remove_container(&mut self, id: &str) -> bool {
        let before = self.containers.len();
        self.containers.retain(|c| c.id != id);
        let removed = self.containers.len() < before;
        if removed {
            self.on_did_change.fire(&());
        }
        removed
    }

    /// Remove a view by id. Returns `true` if it was found and removed.
    pub fn remove_view(&mut self, id: &str) -> bool {
        let before = self.views.len();
        self.views.retain(|v| v.id != id);
        let removed = self.views.len() < before;
        if removed {
            self.on_did_change.fire(&());
        }
        removed
    }

    /// Return the ids of all registered containers.
    pub fn all_container_ids(&self) -> Vec<&str> {
        self.containers.iter().map(|c| c.id.as_str()).collect()
    }

    /// Return the ids of all registered views.
    pub fn all_view_ids(&self) -> Vec<&str> {
        self.views.iter().map(|v| v.id.as_str()).collect()
    }

    /// Get all views whose container is at the given location.
    pub fn views_in_location(&self, location: ViewContainerLocation) -> Vec<&ViewDescriptor> {
        let container_ids: Vec<&str> = self
            .containers
            .iter()
            .filter(|c| c.location == location)
            .map(|c| c.id.as_str())
            .collect();
        self.views
            .iter()
            .filter(|v| container_ids.contains(&v.container_id.as_str()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Default containers
// ---------------------------------------------------------------------------

/// Register the default view containers that ship with the editor.
pub fn register_default_containers(registry: &mut ViewsRegistry) {
    // Sidebar
    registry.register_container(ViewContainer {
        id: "workbench.view.explorer".into(),
        title: "Explorer".into(),
        icon: Some("files".into()),
        location: ViewContainerLocation::Sidebar,
        order: 0,
    });
    registry.register_container(ViewContainer {
        id: "workbench.view.search".into(),
        title: "Search".into(),
        icon: Some("search".into()),
        location: ViewContainerLocation::Sidebar,
        order: 1,
    });
    registry.register_container(ViewContainer {
        id: "workbench.view.scm".into(),
        title: "Source Control".into(),
        icon: Some("source-control".into()),
        location: ViewContainerLocation::Sidebar,
        order: 2,
    });
    registry.register_container(ViewContainer {
        id: "workbench.view.debug".into(),
        title: "Run and Debug".into(),
        icon: Some("debug-alt".into()),
        location: ViewContainerLocation::Sidebar,
        order: 3,
    });
    registry.register_container(ViewContainer {
        id: "workbench.view.extensions".into(),
        title: "Extensions".into(),
        icon: Some("extensions".into()),
        location: ViewContainerLocation::Sidebar,
        order: 4,
    });

    // Panel
    registry.register_container(ViewContainer {
        id: "workbench.panel.terminal".into(),
        title: "Terminal".into(),
        icon: Some("terminal".into()),
        location: ViewContainerLocation::Panel,
        order: 0,
    });
    registry.register_container(ViewContainer {
        id: "workbench.panel.markers".into(),
        title: "Problems".into(),
        icon: Some("warning".into()),
        location: ViewContainerLocation::Panel,
        order: 1,
    });
    registry.register_container(ViewContainer {
        id: "workbench.panel.output".into(),
        title: "Output".into(),
        icon: Some("output".into()),
        location: ViewContainerLocation::Panel,
        order: 2,
    });
    registry.register_container(ViewContainer {
        id: "workbench.panel.repl".into(),
        title: "Debug Console".into(),
        icon: Some("debug-console".into()),
        location: ViewContainerLocation::Panel,
        order: 3,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_container(id: &str, location: ViewContainerLocation, order: i32) -> ViewContainer {
        ViewContainer {
            id: id.into(),
            title: id.into(),
            icon: None,
            location,
            order,
        }
    }

    fn make_view(id: &str, container_id: &str, order: i32) -> ViewDescriptor {
        ViewDescriptor {
            id: id.into(),
            name: id.into(),
            container_id: container_id.into(),
            when: None,
            order,
            can_toggle_visibility: true,
        }
    }

    #[test]
    fn register_and_get_containers() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("b", ViewContainerLocation::Sidebar, 2));
        reg.register_container(make_container("a", ViewContainerLocation::Sidebar, 1));
        reg.register_container(make_container("p", ViewContainerLocation::Panel, 0));

        let sidebar = reg.get_containers(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar.len(), 2);
        assert_eq!(sidebar[0].id, "a");
        assert_eq!(sidebar[1].id, "b");

        let panel = reg.get_containers(ViewContainerLocation::Panel);
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].id, "p");

        let aux = reg.get_containers(ViewContainerLocation::AuxiliaryBar);
        assert!(aux.is_empty());
    }

    #[test]
    fn register_and_get_views() {
        let mut reg = ViewsRegistry::new();
        reg.register_view(make_view("v2", "explorer", 2));
        reg.register_view(make_view("v1", "explorer", 1));
        reg.register_view(make_view("v3", "scm", 0));

        let explorer_views = reg.get_views("explorer");
        assert_eq!(explorer_views.len(), 2);
        assert_eq!(explorer_views[0].id, "v1");
        assert_eq!(explorer_views[1].id, "v2");

        let scm_views = reg.get_views("scm");
        assert_eq!(scm_views.len(), 1);
        assert_eq!(scm_views[0].id, "v3");

        assert!(reg.get_views("nonexistent").is_empty());
    }

    #[test]
    fn active_container_tracking() {
        let mut reg = ViewsRegistry::new();
        assert!(reg.get_active_container(ViewContainerLocation::Sidebar).is_none());

        reg.set_active_container(ViewContainerLocation::Sidebar, "explorer");
        assert_eq!(
            reg.get_active_container(ViewContainerLocation::Sidebar),
            Some("explorer")
        );

        reg.set_active_container(ViewContainerLocation::Sidebar, "search");
        assert_eq!(
            reg.get_active_container(ViewContainerLocation::Sidebar),
            Some("search")
        );

        // Panel is still unset
        assert!(reg.get_active_container(ViewContainerLocation::Panel).is_none());
    }

    #[test]
    fn on_did_change_fires() {
        let mut reg = ViewsRegistry::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _handle = reg.on_did_change().on(move |_| {
            *c.lock().unwrap() += 1;
        });

        reg.register_container(make_container("a", ViewContainerLocation::Sidebar, 0));
        reg.register_view(make_view("v", "a", 0));
        reg.set_active_container(ViewContainerLocation::Sidebar, "a");

        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn default_containers() {
        let mut reg = ViewsRegistry::new();
        register_default_containers(&mut reg);

        let sidebar = reg.get_containers(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar.len(), 5);
        assert_eq!(sidebar[0].title, "Explorer");
        assert_eq!(sidebar[4].title, "Extensions");

        let panel = reg.get_containers(ViewContainerLocation::Panel);
        assert_eq!(panel.len(), 4);
        assert_eq!(panel[0].title, "Terminal");
        assert_eq!(panel[3].title, "Debug Console");
    }

    #[test]
    fn test_view_container_location_display() {
        assert_eq!(ViewContainerLocation::Sidebar.to_string(), "Sidebar");
        assert_eq!(ViewContainerLocation::Panel.to_string(), "Panel");
        assert_eq!(
            ViewContainerLocation::AuxiliaryBar.to_string(),
            "AuxiliaryBar"
        );
    }

    #[test]
    fn test_view_container_display() {
        let c = ViewContainer {
            id: "explorer".into(),
            title: "Explorer".into(),
            icon: None,
            location: ViewContainerLocation::Sidebar,
            order: 0,
        };
        assert_eq!(c.to_string(), "Explorer [Sidebar]");
    }

    #[test]
    fn test_view_descriptor_display() {
        let v = ViewDescriptor {
            id: "files".into(),
            name: "File Explorer".into(),
            container_id: "explorer".into(),
            when: None,
            order: 0,
            can_toggle_visibility: true,
        };
        assert_eq!(v.to_string(), "File Explorer (in explorer)");
    }

    #[test]
    fn test_view_visibility_defaults() {
        let vis = ViewVisibility::new();
        assert!(vis.is_visible("anything"));
        assert!(vis.hidden_views().is_empty());
        assert_eq!(vis.visible_count(), 0);
    }

    #[test]
    fn test_view_visibility_set_and_check() {
        let mut vis = ViewVisibility::new();
        vis.set_visible("a", true);
        vis.set_visible("b", false);
        assert!(vis.is_visible("a"));
        assert!(!vis.is_visible("b"));
        assert!(vis.is_visible("c")); // default true
        assert_eq!(vis.visible_count(), 1);
    }

    #[test]
    fn test_view_visibility_hidden_views() {
        let mut vis = ViewVisibility::new();
        vis.set_visible("a", false);
        vis.set_visible("b", true);
        vis.set_visible("c", false);
        let mut hidden = vis.hidden_views();
        hidden.sort();
        assert_eq!(hidden, vec!["a", "c"]);
    }

    #[test]
    fn test_view_visibility_toggle() {
        let mut vis = ViewVisibility::new();
        // Not tracked yet — default is true, toggle makes it false
        vis.toggle("x");
        assert!(!vis.is_visible("x"));
        // Toggle again — back to true
        vis.toggle("x");
        assert!(vis.is_visible("x"));
    }

    #[test]
    fn test_view_visibility_reset() {
        let mut vis = ViewVisibility::new();
        vis.set_visible("a", false);
        vis.set_visible("b", true);
        vis.reset();
        assert!(vis.is_visible("a"));
        assert!(vis.is_visible("b"));
        assert_eq!(vis.visible_count(), 0);
    }

    #[test]
    fn test_container_count_and_view_count() {
        let mut reg = ViewsRegistry::new();
        assert_eq!(reg.container_count(), 0);
        assert_eq!(reg.view_count(), 0);
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_view(make_view("v1", "c1", 0));
        reg.register_view(make_view("v2", "c1", 1));
        assert_eq!(reg.container_count(), 1);
        assert_eq!(reg.view_count(), 2);
    }

    #[test]
    fn test_get_container_and_view() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Panel, 0));
        reg.register_view(make_view("v1", "c1", 0));

        assert!(reg.get_container("c1").is_some());
        assert_eq!(reg.get_container("c1").unwrap().id, "c1");
        assert!(reg.get_container("missing").is_none());

        assert!(reg.get_view("v1").is_some());
        assert_eq!(reg.get_view("v1").unwrap().id, "v1");
        assert!(reg.get_view("missing").is_none());
    }

    #[test]
    fn test_remove_container() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_container(make_container("c2", ViewContainerLocation::Panel, 1));
        assert_eq!(reg.container_count(), 2);

        assert!(reg.remove_container("c1"));
        assert_eq!(reg.container_count(), 1);
        assert!(reg.get_container("c1").is_none());

        // Removing again returns false
        assert!(!reg.remove_container("c1"));
    }

    #[test]
    fn test_remove_view() {
        let mut reg = ViewsRegistry::new();
        reg.register_view(make_view("v1", "c1", 0));
        reg.register_view(make_view("v2", "c1", 1));
        assert_eq!(reg.view_count(), 2);

        assert!(reg.remove_view("v1"));
        assert_eq!(reg.view_count(), 1);
        assert!(reg.get_view("v1").is_none());

        assert!(!reg.remove_view("v1"));
    }

    #[test]
    fn test_all_container_ids_and_view_ids() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_container(make_container("c2", ViewContainerLocation::Panel, 1));
        reg.register_view(make_view("v1", "c1", 0));
        reg.register_view(make_view("v2", "c2", 0));

        let mut cids = reg.all_container_ids();
        cids.sort();
        assert_eq!(cids, vec!["c1", "c2"]);

        let mut vids = reg.all_view_ids();
        vids.sort();
        assert_eq!(vids, vec!["v1", "v2"]);
    }

    #[test]
    fn test_views_in_location() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_container(make_container("c2", ViewContainerLocation::Panel, 0));
        reg.register_view(make_view("v1", "c1", 0));
        reg.register_view(make_view("v2", "c1", 1));
        reg.register_view(make_view("v3", "c2", 0));

        let sidebar_views = reg.views_in_location(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar_views.len(), 2);

        let panel_views = reg.views_in_location(ViewContainerLocation::Panel);
        assert_eq!(panel_views.len(), 1);
        assert_eq!(panel_views[0].id, "v3");

        let aux_views = reg.views_in_location(ViewContainerLocation::AuxiliaryBar);
        assert!(aux_views.is_empty());
    }
}
