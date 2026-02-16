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

/// Accumulated statistics for wb-views operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbViewsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbViewsStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &WbViewsStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for WbViewsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbViewsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbViewsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-views.
#[derive(Debug, Clone)]
pub struct WbViewsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbViewsValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for WbViewsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ViewDescriptor builder
// ---------------------------------------------------------------------------

impl ViewDescriptor {
    /// Create a new view descriptor with required fields.
    pub fn new(id: impl Into<String>, name: impl Into<String>, container_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            container_id: container_id.into(),
            when: None,
            order: 0,
            can_toggle_visibility: true,
        }
    }

    /// Builder method to set the order.
    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Builder method to set the when condition.
    pub fn with_when(mut self, when: ContextKeyExpr) -> Self {
        self.when = Some(when);
        self
    }

    /// Builder method to set toggleable visibility.
    pub fn with_toggle_visibility(mut self, toggle: bool) -> Self {
        self.can_toggle_visibility = toggle;
        self
    }
}

// ---------------------------------------------------------------------------
// ViewVisibilityState
// ---------------------------------------------------------------------------

/// Persisted visibility state for views across sessions.
#[derive(Debug, Clone)]
pub struct ViewVisibilityState {
    state: HashMap<String, bool>,
    dirty: bool,
}

impl ViewVisibilityState {
    /// Create a new empty visibility state.
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
            dirty: false,
        }
    }

    /// Load visibility state from a list of (view_id, visible) pairs.
    pub fn load(pairs: &[(&str, bool)]) -> Self {
        let state = pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        Self { state, dirty: false }
    }

    /// Set visibility for a view.
    pub fn set(&mut self, view_id: impl Into<String>, visible: bool) {
        self.state.insert(view_id.into(), visible);
        self.dirty = true;
    }

    /// Get visibility for a view (defaults to `true`).
    pub fn get(&self, view_id: &str) -> bool {
        self.state.get(view_id).copied().unwrap_or(true)
    }

    /// Toggle visibility for a view.
    pub fn toggle(&mut self, view_id: &str) {
        let current = self.get(view_id);
        self.set(view_id.to_string(), !current);
    }

    /// Whether any changes have been made since load.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Export the state as a vector of (view_id, visible) pairs.
    pub fn export(&self) -> Vec<(String, bool)> {
        let mut pairs: Vec<(String, bool)> = self.state.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    /// Number of tracked views.
    pub fn tracked_count(&self) -> usize {
        self.state.len()
    }

    /// Mark the state as clean (after persisting).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

impl Default for ViewVisibilityState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve visibility state for a view, returning true if visible.
pub fn view_visibility_state(state: &ViewVisibilityState, view_id: &str) -> bool {
    state.get(view_id)
}

// ---------------------------------------------------------------------------
// ViewDragData
// ---------------------------------------------------------------------------

/// Data transferred during a view drag operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDragData {
    pub view_id: String,
    pub source_container: String,
    pub target_container: Option<String>,
}

impl ViewDragData {
    /// Create new drag data for a view.
    pub fn new(view_id: impl Into<String>, source_container: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            source_container: source_container.into(),
            target_container: None,
        }
    }

    /// Set the target container.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target_container = Some(target.into());
        self
    }

    /// Check if the drag has a valid target (different from source).
    pub fn is_valid_drop(&self) -> bool {
        match &self.target_container {
            Some(target) => target != &self.source_container,
            None => false,
        }
    }
}

impl ViewsRegistry {
    /// Move a view to a different container.
    pub fn move_view_to_container(&mut self, view_id: &str, new_container_id: &str) -> bool {
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.container_id = new_container_id.to_string();
            self.on_did_change.fire(&());
            true
        } else {
            false
        }
    }
}

/// Execute a view drag transfer operation on the registry.
/// Moves the view from its current container to the target container.
pub fn view_drag_transfer(
    registry: &mut ViewsRegistry,
    drag_data: &ViewDragData,
) -> bool {
    if !drag_data.is_valid_drop() {
        return false;
    }
    let target = match &drag_data.target_container {
        Some(t) => t.clone(),
        None => return false,
    };
    registry.move_view_to_container(&drag_data.view_id, &target)
}

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

    #[test]
    fn wb_views_stats_new_defaults() {
        let stats = WbViewsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_views_stats_record_success() {
        let mut stats = WbViewsStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_views_stats_record_failure() {
        let mut stats = WbViewsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_views_stats_reset() {
        let mut stats = WbViewsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_views_stats_merge() {
        let mut a = WbViewsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbViewsStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn wb_views_stats_display() {
        let mut stats = WbViewsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_views_stats_default() {
        let stats = WbViewsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_views_validator_accepts_valid_name() {
        let v = WbViewsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_views_validator_rejects_empty() {
        let v = WbViewsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_views_validator_rejects_too_long() {
        let v = WbViewsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_views_validator_forbidden_prefix() {
        let v = WbViewsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_views_validator_allowed_chars() {
        let v = WbViewsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_views_validator_range() {
        let v = WbViewsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_views_sanitize_removes_control() {
        let result = WbViewsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_views_truncate_short_string() {
        assert_eq!(WbViewsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_views_truncate_long_string() {
        let result = WbViewsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_views_is_ascii_printable() {
        assert!(WbViewsValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbViewsValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn view_descriptor_builder() {
        let vd = ViewDescriptor::new("files", "Files", "explorer")
            .with_order(1)
            .with_toggle_visibility(false);
        assert_eq!(vd.id, "files");
        assert_eq!(vd.order, 1);
        assert!(!vd.can_toggle_visibility);
    }

    #[test]
    fn view_visibility_state_basic() {
        let mut state = ViewVisibilityState::new();
        assert!(state.get("any")); // default true
        state.set("panel", false);
        assert!(!state.get("panel"));
        assert!(state.is_dirty());
    }

    #[test]
    fn view_visibility_state_toggle() {
        let mut state = ViewVisibilityState::new();
        state.toggle("panel");
        assert!(!state.get("panel")); // was true, now false
        state.toggle("panel");
        assert!(state.get("panel")); // back to true
    }

    #[test]
    fn view_visibility_state_load_and_export() {
        let state = ViewVisibilityState::load(&[("a", true), ("b", false)]);
        let exported = state.export();
        assert_eq!(exported.len(), 2);
        assert!(!state.is_dirty());
    }

    #[test]
    fn view_visibility_state_fn() {
        let mut state = ViewVisibilityState::new();
        state.set("panel", false);
        assert!(!view_visibility_state(&state, "panel"));
        assert!(view_visibility_state(&state, "unknown"));
    }

    #[test]
    fn view_drag_data_valid_drop() {
        let drag = ViewDragData::new("files", "explorer")
            .with_target("panel");
        assert!(drag.is_valid_drop());
    }

    #[test]
    fn view_drag_data_same_container() {
        let drag = ViewDragData::new("files", "explorer")
            .with_target("explorer");
        assert!(!drag.is_valid_drop());
    }

    #[test]
    fn view_drag_transfer_moves_view() {
        let mut registry = ViewsRegistry::new();
        registry.register_container(ViewContainer {
            id: "explorer".into(),
            title: "Explorer".into(),
            icon: None,
            location: ViewContainerLocation::Sidebar,
            order: 0,
        });
        registry.register_container(ViewContainer {
            id: "panel".into(),
            title: "Panel".into(),
            icon: None,
            location: ViewContainerLocation::Panel,
            order: 0,
        });
        registry.register_view(ViewDescriptor::new("files", "Files", "explorer"));
        let drag = ViewDragData::new("files", "explorer").with_target("panel");
        assert!(view_drag_transfer(&mut registry, &drag));
        let views = registry.get_views("panel");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, "files");
    }
}
