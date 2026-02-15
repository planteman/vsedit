//! View container system — manages view containers and views.
//!
//! This crate provides the view registration service equivalent to VS Code's
//! views system, including [`ViewContainer`], [`ViewDescriptor`], and the
//! central [`ViewsRegistry`].

use std::collections::HashMap;

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
}
