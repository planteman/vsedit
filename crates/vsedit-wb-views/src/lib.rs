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

// ---------------------------------------------------------------------------
// ViewStack
// ---------------------------------------------------------------------------

/// A stack-based manager for view layers with push/pop semantics.
#[derive(Debug, Clone)]
pub struct ViewStack {
    layers: Vec<String>,
    max_depth: usize,
}

impl ViewStack {
    /// Create a new view stack with a maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            layers: Vec::new(),
            max_depth: max_depth.max(1),
        }
    }

    /// Push a view onto the stack. Returns false if the stack is full.
    pub fn push(&mut self, view_id: impl Into<String>) -> bool {
        if self.layers.len() >= self.max_depth {
            return false;
        }
        self.layers.push(view_id.into());
        true
    }

    /// Pop the top view off the stack.
    pub fn pop(&mut self) -> Option<String> {
        self.layers.pop()
    }

    /// Peek at the top view without removing it.
    pub fn top(&self) -> Option<&str> {
        self.layers.last().map(|s| s.as_str())
    }

    /// Number of views on the stack.
    pub fn depth(&self) -> usize {
        self.layers.len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Check if the stack is full.
    pub fn is_full(&self) -> bool {
        self.layers.len() >= self.max_depth
    }

    /// Check if a view is anywhere in the stack.
    pub fn contains(&self, view_id: &str) -> bool {
        self.layers.iter().any(|v| v == view_id)
    }

    /// Clear all views from the stack.
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    /// Get all views as a slice, bottom to top.
    pub fn as_slice(&self) -> &[String] {
        &self.layers
    }

    /// Bring a specific view to the top, removing it from its current position.
    pub fn bring_to_top(&mut self, view_id: &str) -> bool {
        if let Some(pos) = self.layers.iter().position(|v| v == view_id) {
            let view = self.layers.remove(pos);
            self.layers.push(view);
            true
        } else {
            false
        }
    }
}

impl Default for ViewStack {
    fn default() -> Self {
        Self::new(64)
    }
}

// ---------------------------------------------------------------------------
// ViewFocusHistory
// ---------------------------------------------------------------------------

/// Tracks which views have been focused, in order.
#[derive(Debug, Clone)]
pub struct ViewFocusHistory {
    history: Vec<String>,
    max_entries: usize,
}

impl ViewFocusHistory {
    /// Create a new focus history with a maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            history: Vec::new(),
            max_entries: max_entries.max(1),
        }
    }

    /// Record that a view received focus.
    /// If already in history, it's moved to the most recent position.
    pub fn record_focus(&mut self, view_id: impl Into<String>) {
        let id = view_id.into();
        self.history.retain(|v| v != &id);
        self.history.push(id);
        while self.history.len() > self.max_entries {
            self.history.remove(0);
        }
    }

    /// Get the most recently focused view.
    pub fn most_recent(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Get the previously focused view (the one before the most recent).
    pub fn previous(&self) -> Option<&str> {
        if self.history.len() >= 2 {
            Some(self.history[self.history.len() - 2].as_str())
        } else {
            None
        }
    }

    /// Number of entries in the history.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Get the full focus history, most recent last.
    pub fn entries(&self) -> &[String] {
        &self.history
    }

    /// Clear the focus history.
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Check if a view appears in the history.
    pub fn contains(&self, view_id: &str) -> bool {
        self.history.iter().any(|v| v == view_id)
    }
}

impl Default for ViewFocusHistory {
    fn default() -> Self {
        Self::new(50)
    }
}

// ---------------------------------------------------------------------------
// ViewLayoutConstraint
// ---------------------------------------------------------------------------

/// Layout constraints for a view with aspect ratio support.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewLayoutConstraint {
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
    pub aspect_ratio: Option<f64>,
}

impl ViewLayoutConstraint {
    /// Create constraints with minimum dimensions.
    pub fn new(min_width: u16, min_height: u16) -> Self {
        Self {
            min_width,
            min_height,
            max_width: None,
            max_height: None,
            aspect_ratio: None,
        }
    }

    /// Set the desired aspect ratio (width / height).
    pub fn with_aspect_ratio(mut self, ratio: f64) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    /// Set the maximum width.
    pub fn with_max_width(mut self, w: u16) -> Self {
        self.max_width = Some(w);
        self
    }

    /// Set the maximum height.
    pub fn with_max_height(mut self, h: u16) -> Self {
        self.max_height = Some(h);
        self
    }

    /// Given a width, compute the constrained height respecting aspect ratio.
    pub fn compute_height_for_width(&self, width: u16) -> u16 {
        let h = match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => (width as f64 / ratio).round() as u16,
            _ => self.min_height,
        };
        let h = h.max(self.min_height);
        match self.max_height {
            Some(max) => h.min(max),
            None => h,
        }
    }

    /// Given a height, compute the constrained width respecting aspect ratio.
    pub fn compute_width_for_height(&self, height: u16) -> u16 {
        let w = match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => (height as f64 * ratio).round() as u16,
            _ => self.min_width,
        };
        let w = w.max(self.min_width);
        match self.max_width {
            Some(max) => w.min(max),
            None => w,
        }
    }

    /// Check if given dimensions satisfy these constraints.
    pub fn satisfies(&self, width: u16, height: u16) -> bool {
        if width < self.min_width || height < self.min_height {
            return false;
        }
        if self.max_width.is_some_and(|m| width > m) {
            return false;
        }
        if self.max_height.is_some_and(|m| height > m) {
            return false;
        }
        true
    }
}

impl Default for ViewLayoutConstraint {
    fn default() -> Self {
        Self::new(1, 1)
    }
}

// ---------------------------------------------------------------------------
// Extended ViewsRegistry methods
// ---------------------------------------------------------------------------

impl ViewsRegistry {
    /// Unregister a view by id, returning the removed descriptor if found.
    pub fn unregister_view(&mut self, id: &str) -> Option<ViewDescriptor> {
        let pos = self.views.iter().position(|v| v.id == id)?;
        let removed = self.views.remove(pos);
        self.on_did_change.fire(&());
        Some(removed)
    }

    /// Move a view to a new position (order) within its current container.
    ///
    /// Returns `false` if the view was not found.
    pub fn move_view(&mut self, view_id: &str, new_order: i32) -> bool {
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.order = new_order;
            self.on_did_change.fire(&());
            true
        } else {
            false
        }
    }

    /// Find views whose name contains the given substring (case-insensitive).
    pub fn find_view_by_name(&self, needle: &str) -> Vec<&ViewDescriptor> {
        let lower = needle.to_lowercase();
        self.views
            .iter()
            .filter(|v| v.name.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return the number of containers at a given location.
    pub fn container_count_at(&self, location: ViewContainerLocation) -> usize {
        self.containers
            .iter()
            .filter(|c| c.location == location)
            .count()
    }

    /// Return all views sorted by their name alphabetically.
    pub fn views_sorted_by_name(&self) -> Vec<&ViewDescriptor> {
        let mut sorted: Vec<&ViewDescriptor> = self.views.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        sorted
    }
}

impl ViewVisibility {
    /// Set all tracked views to visible.
    pub fn show_all(&mut self) {
        for val in self.visibility.values_mut() {
            *val = true;
        }
    }

    /// Set all tracked views to hidden.
    pub fn hide_all(&mut self) {
        for val in self.visibility.values_mut() {
            *val = false;
        }
    }

    /// Return the total number of tracked view entries.
    pub fn tracked_count(&self) -> usize {
        self.visibility.len()
    }
}

impl ViewDescriptor {
    /// Compare two view descriptors by order, breaking ties alphabetically by name.
    pub fn cmp_by_order_then_name(&self, other: &ViewDescriptor) -> std::cmp::Ordering {
        self.order
            .cmp(&other.order)
            .then_with(|| self.name.cmp(&other.name))
    }
}

// ---------------------------------------------------------------------------
// ViewBadge
// ---------------------------------------------------------------------------

/// A badge displayed on a view, typically showing a count or short label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewBadge {
    pub view_id: String,
    pub value: ViewBadgeValue,
}

/// The content of a view badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewBadgeValue {
    /// A numeric counter (e.g. unread items, problems).
    Count(u32),
    /// A short text label (e.g. "!").
    Label(String),
    /// No badge — used to clear.
    None,
}

impl ViewBadge {
    /// Create a counter badge for a view.
    pub fn count(view_id: impl Into<String>, n: u32) -> Self {
        Self {
            view_id: view_id.into(),
            value: ViewBadgeValue::Count(n),
        }
    }

    /// Create a label badge for a view.
    pub fn label(view_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            value: ViewBadgeValue::Label(text.into()),
        }
    }

    /// Create an empty badge (clears the badge).
    pub fn none(view_id: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            value: ViewBadgeValue::None,
        }
    }

    /// Whether this badge has displayable content.
    pub fn is_visible(&self) -> bool {
        match &self.value {
            ViewBadgeValue::Count(n) => *n > 0,
            ViewBadgeValue::Label(s) => !s.is_empty(),
            ViewBadgeValue::None => false,
        }
    }

    /// Return display text for the badge.
    pub fn display_text(&self) -> Option<String> {
        match &self.value {
            ViewBadgeValue::Count(0) | ViewBadgeValue::None => None,
            ViewBadgeValue::Count(n) => Some(n.to_string()),
            ViewBadgeValue::Label(s) if s.is_empty() => None,
            ViewBadgeValue::Label(s) => Some(s.clone()),
        }
    }

    /// Increment a counter badge. No-op for label/none badges.
    pub fn increment(&mut self) {
        if let ViewBadgeValue::Count(ref mut n) = self.value {
            *n = n.saturating_add(1);
        }
    }

    /// Decrement a counter badge (saturating at 0). No-op for label/none.
    pub fn decrement(&mut self) {
        if let ViewBadgeValue::Count(ref mut n) = self.value {
            *n = n.saturating_sub(1);
        }
    }
}

impl fmt::Display for ViewBadge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            ViewBadgeValue::Count(n) => write!(f, "[{}]({})", self.view_id, n),
            ViewBadgeValue::Label(s) => write!(f, "[{}]({})", self.view_id, s),
            ViewBadgeValue::None => write!(f, "[{}](none)", self.view_id),
        }
    }
}

// ---------------------------------------------------------------------------
// ViewBadgeManager
// ---------------------------------------------------------------------------

/// Manages badges across all views.
pub struct ViewBadgeManager {
    badges: HashMap<String, ViewBadge>,
    on_did_change: Emitter<String>,
}

impl ViewBadgeManager {
    /// Create a new badge manager.
    pub fn new() -> Self {
        Self {
            badges: HashMap::new(),
            on_did_change: Emitter::new(),
        }
    }

    /// Set or update a badge for a view. Fires a change event.
    pub fn set_badge(&mut self, badge: ViewBadge) {
        let view_id = badge.view_id.clone();
        self.badges.insert(view_id.clone(), badge);
        self.on_did_change.fire(&view_id);
    }

    /// Get the badge for a view, if any.
    pub fn get_badge(&self, view_id: &str) -> Option<&ViewBadge> {
        self.badges.get(view_id)
    }

    /// Remove the badge for a view. Returns `true` if one existed.
    pub fn clear_badge(&mut self, view_id: &str) -> bool {
        let removed = self.badges.remove(view_id).is_some();
        if removed {
            self.on_did_change.fire(&view_id.to_string());
        }
        removed
    }

    /// Clear all badges.
    pub fn clear_all(&mut self) {
        self.badges.clear();
    }

    /// Return all view ids that currently have a visible badge.
    pub fn views_with_badges(&self) -> Vec<&str> {
        self.badges
            .iter()
            .filter(|(_, b)| b.is_visible())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Total count across all counter badges.
    pub fn total_count(&self) -> u32 {
        self.badges.values().fold(0u32, |acc, b| match &b.value {
            ViewBadgeValue::Count(n) => acc.saturating_add(*n),
            _ => acc,
        })
    }

    /// Subscribe to badge change events. The event payload is the view id.
    pub fn on_did_change(&self) -> Event<String> {
        self.on_did_change.event()
    }
}

impl Default for ViewBadgeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WelcomeViewContent
// ---------------------------------------------------------------------------

/// A single content item for a view's welcome/empty-state page.
#[derive(Debug, Clone, PartialEq)]
pub struct WelcomeViewItem {
    pub content: String,
    pub when: Option<ContextKeyExpr>,
    pub order: i32,
}

/// Manages welcome view content for views.
#[derive(Debug, Clone)]
pub struct WelcomeViewContent {
    items: HashMap<String, Vec<WelcomeViewItem>>,
}

impl WelcomeViewContent {
    /// Create a new empty welcome-view content manager.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Add a content item to a view's welcome page.
    pub fn add_item(&mut self, view_id: &str, item: WelcomeViewItem) {
        self.items
            .entry(view_id.to_string())
            .or_default()
            .push(item);
    }

    /// Get all welcome items for a view, sorted by order.
    pub fn get_items(&self, view_id: &str) -> Vec<&WelcomeViewItem> {
        match self.items.get(view_id) {
            Some(items) => {
                let mut sorted: Vec<&WelcomeViewItem> = items.iter().collect();
                sorted.sort_by_key(|i| i.order);
                sorted
            }
            None => Vec::new(),
        }
    }

    /// Remove all welcome items for a view.
    pub fn clear_view(&mut self, view_id: &str) {
        self.items.remove(view_id);
    }

    /// Check if a view has any welcome content.
    pub fn has_content(&self, view_id: &str) -> bool {
        self.items
            .get(view_id)
            .map_or(false, |items| !items.is_empty())
    }

    /// Count how many views have welcome content.
    pub fn view_count_with_content(&self) -> usize {
        self.items.values().filter(|items| !items.is_empty()).count()
    }
}

impl Default for WelcomeViewContent {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ViewContainerModel — tracks runtime state for a single container
// ---------------------------------------------------------------------------

/// Runtime model for an active view container, tracking its views' state.
pub struct ViewContainerModel {
    container_id: String,
    visible_views: Vec<String>,
    collapsed: HashMap<String, bool>,
    active_view: Option<String>,
    on_did_change: Emitter<()>,
}

impl ViewContainerModel {
    /// Create a model for a container with initial view ids.
    pub fn new(container_id: impl Into<String>, view_ids: &[&str]) -> Self {
        Self {
            container_id: container_id.into(),
            visible_views: view_ids.iter().map(|s| s.to_string()).collect(),
            collapsed: HashMap::new(),
            active_view: view_ids.first().map(|s| s.to_string()),
            on_did_change: Emitter::new(),
        }
    }

    /// The container this model is for.
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Whether a view is collapsed (tree folded).
    pub fn is_collapsed(&self, view_id: &str) -> bool {
        self.collapsed.get(view_id).copied().unwrap_or(false)
    }

    /// Set the collapsed state for a view.
    pub fn set_collapsed(&mut self, view_id: &str, collapsed: bool) {
        self.collapsed.insert(view_id.to_string(), collapsed);
        self.on_did_change.fire(&());
    }

    /// Toggle collapse for a view.
    pub fn toggle_collapsed(&mut self, view_id: &str) {
        let current = self.is_collapsed(view_id);
        self.set_collapsed(view_id, !current);
    }

    /// Get the currently active (focused) view in this container.
    pub fn active_view(&self) -> Option<&str> {
        self.active_view.as_deref()
    }

    /// Set which view is active in this container.
    pub fn set_active_view(&mut self, view_id: &str) {
        if self.visible_views.iter().any(|v| v == view_id) {
            self.active_view = Some(view_id.to_string());
            self.on_did_change.fire(&());
        }
    }

    /// Get the ordered list of visible view ids.
    pub fn visible_views(&self) -> &[String] {
        &self.visible_views
    }

    /// Hide a view (remove from visible list). Returns `true` if found.
    pub fn hide_view(&mut self, view_id: &str) -> bool {
        let before = self.visible_views.len();
        self.visible_views.retain(|v| v != view_id);
        let removed = self.visible_views.len() < before;
        if removed {
            if self.active_view.as_deref() == Some(view_id) {
                self.active_view = self.visible_views.first().cloned();
            }
            self.on_did_change.fire(&());
        }
        removed
    }

    /// Show a view (append to visible list if not already present).
    pub fn show_view(&mut self, view_id: &str) {
        if !self.visible_views.iter().any(|v| v == view_id) {
            self.visible_views.push(view_id.to_string());
            self.on_did_change.fire(&());
        }
    }

    /// Reorder a view to a new index. Returns false if view not found or
    /// index is out of bounds.
    pub fn move_view_to_index(&mut self, view_id: &str, new_index: usize) -> bool {
        let pos = match self.visible_views.iter().position(|v| v == view_id) {
            Some(p) => p,
            None => return false,
        };
        if new_index >= self.visible_views.len() {
            return false;
        }
        let v = self.visible_views.remove(pos);
        self.visible_views.insert(new_index, v);
        self.on_did_change.fire(&());
        true
    }

    /// Number of currently visible views.
    pub fn visible_count(&self) -> usize {
        self.visible_views.len()
    }

    /// Subscribe to model change events.
    pub fn on_did_change(&self) -> Event<()> {
        self.on_did_change.event()
    }
}


// === View Container Organizer ===

/// View Container Organizer implementation.
#[derive(Debug, Clone)]
pub struct ViewContainerOrganizer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ViewContainerOrganizerStats,
}

/// Statistics for ViewContainerOrganizer.
#[derive(Debug, Clone, Default)]
pub struct ViewContainerOrganizerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ViewContainerOrganizerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl ViewContainerOrganizer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ViewContainerOrganizerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &ViewContainerOrganizerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for ViewContainerOrganizer {
    fn default() -> Self {
        Self::new()
    }
}

// === View Visibility Toggle ===

/// Priority level for ViewVisibilityToggle items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewVisibilityTogglePriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ViewVisibilityTogglePriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ViewVisibilityTogglePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// View Visibility Toggle implementation.
#[derive(Debug, Clone)]
pub struct ViewVisibilityToggle {
    items: Vec<ViewVisibilityToggleItem>,
    max_items: usize,
    default_priority: ViewVisibilityTogglePriority,
}

/// A single item in ViewVisibilityToggle.
#[derive(Debug, Clone)]
pub struct ViewVisibilityToggleItem {
    pub id: String,
    pub label: String,
    pub priority: ViewVisibilityTogglePriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ViewVisibilityToggleItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ViewVisibilityTogglePriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ViewVisibilityTogglePriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl ViewVisibilityToggle {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ViewVisibilityTogglePriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ViewVisibilityToggleItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ViewVisibilityToggleItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ViewVisibilityToggleItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: ViewVisibilityTogglePriority) -> Vec<&ViewVisibilityToggleItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ViewVisibilityToggleItem> {
        let mut sorted: Vec<&ViewVisibilityToggleItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ViewVisibilityToggleItem> {
        let mut sorted: Vec<&ViewVisibilityToggleItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ViewVisibilityToggleItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ViewVisibilityTogglePriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ViewVisibilityTogglePriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ViewVisibilityToggleItem> {
        self.items.iter()
    }
}

impl Default for ViewVisibilityToggle {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench view configuration manager.
#[derive(Debug, Clone)]
pub struct WbViewsConfig {
    entries: Vec<WbViewsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench view entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbViewsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbViewsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbViewsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbViewsEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbViewsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbViewsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbViewsEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbViewsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbViewsEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbViewsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// View container registration — extended utilities (yy)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_views operations.
#[derive(Debug, Clone)]
pub struct YyMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YyMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_views.
#[derive(Debug, Clone)]
pub struct YyRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YyRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_views lookups.
#[derive(Debug, Clone)]
pub struct YyLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YyLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
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

    // ── ViewStack / FocusHistory / LayoutConstraint tests ──

    #[test]
    fn view_stack_push_pop() {
        let mut stack = ViewStack::new(3);
        assert!(stack.push("a"));
        assert!(stack.push("b"));
        assert!(stack.push("c"));
        assert!(!stack.push("d")); // full
        assert!(stack.is_full());
        assert_eq!(stack.top(), Some("c"));
        assert_eq!(stack.pop(), Some("c".to_string()));
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn view_stack_bring_to_top() {
        let mut stack = ViewStack::new(10);
        stack.push("a");
        stack.push("b");
        stack.push("c");
        assert!(stack.bring_to_top("a"));
        assert_eq!(stack.top(), Some("a"));
        assert!(!stack.bring_to_top("nonexistent"));
    }

    #[test]
    fn focus_history_tracks_order() {
        let mut hist = ViewFocusHistory::new(5);
        hist.record_focus("editor");
        hist.record_focus("terminal");
        hist.record_focus("files");
        assert_eq!(hist.most_recent(), Some("files"));
        assert_eq!(hist.previous(), Some("terminal"));
        assert_eq!(hist.len(), 3);
    }

    #[test]
    fn focus_history_deduplicates() {
        let mut hist = ViewFocusHistory::new(10);
        hist.record_focus("a");
        hist.record_focus("b");
        hist.record_focus("a"); // should move "a" to most recent
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.most_recent(), Some("a"));
    }

    #[test]
    fn focus_history_max_entries() {
        let mut hist = ViewFocusHistory::new(2);
        hist.record_focus("a");
        hist.record_focus("b");
        hist.record_focus("c");
        assert_eq!(hist.len(), 2);
        assert!(!hist.contains("a"));
        assert!(hist.contains("b"));
        assert!(hist.contains("c"));
    }

    #[test]
    fn view_layout_constraint_aspect_ratio() {
        let c = ViewLayoutConstraint::new(10, 10).with_aspect_ratio(16.0 / 9.0);
        let h = c.compute_height_for_width(160);
        assert_eq!(h, 90);
        let w = c.compute_width_for_height(90);
        assert_eq!(w, 160);
    }

    #[test]
    fn view_layout_constraint_satisfies() {
        let c = ViewLayoutConstraint::new(10, 10)
            .with_max_width(200)
            .with_max_height(150);
        assert!(c.satisfies(100, 100));
        assert!(!c.satisfies(5, 100));
        assert!(!c.satisfies(100, 5));
        assert!(!c.satisfies(201, 100));
    }

    // -- New functionality tests --

    #[test]
    fn unregister_view_returns_descriptor() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_view(make_view("v1", "c1", 0));
        reg.register_view(make_view("v2", "c1", 1));
        let removed = reg.unregister_view("v1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "v1");
        assert_eq!(reg.view_count(), 1);
    }

    #[test]
    fn unregister_view_not_found() {
        let mut reg = ViewsRegistry::new();
        assert!(reg.unregister_view("missing").is_none());
    }

    #[test]
    fn move_view_changes_order() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_view(make_view("v1", "c1", 0));
        assert!(reg.move_view("v1", 10));
        assert_eq!(reg.get_view("v1").unwrap().order, 10);
    }

    #[test]
    fn find_view_by_name_case_insensitive() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        let mut v = make_view("v1", "c1", 0);
        v.name = "File Explorer".into();
        reg.register_view(v);
        let results = reg.find_view_by_name("explorer");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "v1");
    }

    #[test]
    fn container_count_at_location() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        reg.register_container(make_container("c2", ViewContainerLocation::Sidebar, 1));
        reg.register_container(make_container("c3", ViewContainerLocation::Panel, 0));
        assert_eq!(reg.container_count_at(ViewContainerLocation::Sidebar), 2);
        assert_eq!(reg.container_count_at(ViewContainerLocation::Panel), 1);
        assert_eq!(reg.container_count_at(ViewContainerLocation::AuxiliaryBar), 0);
    }

    #[test]
    fn views_sorted_by_name() {
        let mut reg = ViewsRegistry::new();
        reg.register_container(make_container("c1", ViewContainerLocation::Sidebar, 0));
        let mut va = make_view("va", "c1", 1);
        va.name = "Zzz".into();
        let mut vb = make_view("vb", "c1", 0);
        vb.name = "Aaa".into();
        reg.register_view(va);
        reg.register_view(vb);
        let sorted = reg.views_sorted_by_name();
        assert_eq!(sorted[0].name, "Aaa");
        assert_eq!(sorted[1].name, "Zzz");
    }

    #[test]
    fn visibility_show_all() {
        let mut vis = ViewVisibility::new();
        vis.set_visible("a", false);
        vis.set_visible("b", false);
        vis.set_visible("c", true);
        vis.show_all();
        assert!(vis.is_visible("a"));
        assert!(vis.is_visible("b"));
        assert!(vis.is_visible("c"));
    }

    #[test]
    fn visibility_hide_all() {
        let mut vis = ViewVisibility::new();
        vis.set_visible("a", true);
        vis.set_visible("b", true);
        vis.hide_all();
        assert!(!vis.is_visible("a"));
        assert!(!vis.is_visible("b"));
    }

    #[test]
    fn view_descriptor_cmp_by_order_then_name() {
        let v1 = ViewDescriptor::new("a", "Beta", "c1").with_order(1);
        let v2 = ViewDescriptor::new("b", "Alpha", "c1").with_order(1);
        let v3 = ViewDescriptor::new("c", "Gamma", "c1").with_order(0);
        assert_eq!(v3.cmp_by_order_then_name(&v1), std::cmp::Ordering::Less);
        assert_eq!(v2.cmp_by_order_then_name(&v1), std::cmp::Ordering::Less);
    }

    // -- ViewBadge / ViewBadgeManager tests --

    #[test]
    fn badge_count_visibility_and_display() {
        let mut badge = ViewBadge::count("files", 3);
        assert!(badge.is_visible());
        assert_eq!(badge.display_text(), Some("3".to_string()));
        badge.increment();
        assert_eq!(badge.display_text(), Some("4".to_string()));
        badge.decrement();
        badge.decrement();
        badge.decrement();
        badge.decrement(); // saturates at 0
        assert_eq!(badge.display_text(), None);
        assert!(!badge.is_visible());
    }

    #[test]
    fn badge_label_and_none() {
        let label = ViewBadge::label("scm", "!");
        assert!(label.is_visible());
        assert_eq!(label.display_text(), Some("!".to_string()));
        assert!(label.to_string().contains("!"));

        let empty_label = ViewBadge::label("scm", "");
        assert!(!empty_label.is_visible());
        assert_eq!(empty_label.display_text(), None);

        let none = ViewBadge::none("x");
        assert!(!none.is_visible());
        assert_eq!(none.display_text(), None);
    }

    #[test]
    fn badge_manager_set_get_clear() {
        let mut mgr = ViewBadgeManager::new();
        mgr.set_badge(ViewBadge::count("problems", 5));
        mgr.set_badge(ViewBadge::count("scm", 2));
        assert_eq!(mgr.total_count(), 7);
        assert_eq!(mgr.get_badge("problems").unwrap().display_text(), Some("5".to_string()));

        let mut with_badges = mgr.views_with_badges();
        with_badges.sort();
        assert_eq!(with_badges, vec!["problems", "scm"]);

        assert!(mgr.clear_badge("problems"));
        assert!(!mgr.clear_badge("problems")); // already gone
        assert_eq!(mgr.total_count(), 2);

        mgr.clear_all();
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn badge_manager_fires_change_event() {
        let mut mgr = ViewBadgeManager::new();
        let changed = Arc::new(Mutex::new(Vec::<String>::new()));
        let c = changed.clone();
        let _handle = mgr.on_did_change().on(move |view_id| {
            c.lock().unwrap().push(view_id.clone());
        });
        mgr.set_badge(ViewBadge::count("files", 1));
        mgr.clear_badge("files");
        let events = changed.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "files");
    }

    // -- WelcomeViewContent tests --

    #[test]
    fn welcome_view_content_add_and_retrieve() {
        let mut wv = WelcomeViewContent::new();
        wv.add_item("explorer", WelcomeViewItem {
            content: "Open a folder to get started.".into(),
            when: None,
            order: 1,
        });
        wv.add_item("explorer", WelcomeViewItem {
            content: "Clone a repository.".into(),
            when: None,
            order: 0,
        });
        assert!(wv.has_content("explorer"));
        assert!(!wv.has_content("scm"));
        assert_eq!(wv.view_count_with_content(), 1);

        let items = wv.get_items("explorer");
        assert_eq!(items.len(), 2);
        // Sorted by order
        assert_eq!(items[0].content, "Clone a repository.");
        assert_eq!(items[1].content, "Open a folder to get started.");
    }

    #[test]
    fn welcome_view_content_clear() {
        let mut wv = WelcomeViewContent::new();
        wv.add_item("explorer", WelcomeViewItem {
            content: "Hello".into(),
            when: None,
            order: 0,
        });
        wv.clear_view("explorer");
        assert!(!wv.has_content("explorer"));
        assert!(wv.get_items("explorer").is_empty());
    }

    // -- ViewContainerModel tests --

    #[test]
    fn container_model_active_view_and_collapse() {
        let mut model = ViewContainerModel::new("explorer", &["files", "outline", "timeline"]);
        assert_eq!(model.container_id(), "explorer");
        assert_eq!(model.active_view(), Some("files"));
        assert_eq!(model.visible_count(), 3);

        model.set_active_view("outline");
        assert_eq!(model.active_view(), Some("outline"));

        // Non-existent view is ignored
        model.set_active_view("nonexistent");
        assert_eq!(model.active_view(), Some("outline"));

        assert!(!model.is_collapsed("files"));
        model.toggle_collapsed("files");
        assert!(model.is_collapsed("files"));
        model.toggle_collapsed("files");
        assert!(!model.is_collapsed("files"));
    }

    #[test]
    fn container_model_hide_show_reorder() {
        let mut model = ViewContainerModel::new("explorer", &["files", "outline", "timeline"]);

        // Hide the active view — active moves to next
        assert!(model.hide_view("files"));
        assert_eq!(model.visible_count(), 2);
        assert_eq!(model.active_view(), Some("outline"));
        assert!(!model.hide_view("files")); // already hidden

        // Show it back
        model.show_view("files");
        assert_eq!(model.visible_count(), 3);
        // Appended at end
        assert_eq!(model.visible_views().last().unwrap(), "files");

        // Reorder: move "files" (index 2) to index 0
        assert!(model.move_view_to_index("files", 0));
        assert_eq!(model.visible_views()[0], "files");

        // Out of bounds
        assert!(!model.move_view_to_index("files", 99));
        // Unknown view
        assert!(!model.move_view_to_index("unknown", 0));
    }

    #[test]
    fn container_model_fires_change_events() {
        let mut model = ViewContainerModel::new("c", &["a", "b"]);
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _handle = model.on_did_change().on(move |_| {
            *c.lock().unwrap() += 1;
        });
        model.set_active_view("b");
        model.set_collapsed("a", true);
        model.hide_view("b");
        model.show_view("b");
        model.move_view_to_index("b", 0);
        assert_eq!(*count.lock().unwrap(), 5);
    }

    #[test]
    fn viewContainerOrganizer_new() {
        let s = ViewContainerOrganizer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn viewContainerOrganizer_add_contains() {
        let mut s = ViewContainerOrganizer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn viewContainerOrganizer_add_duplicate() {
        let mut s = ViewContainerOrganizer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn viewContainerOrganizer_remove() {
        let mut s = ViewContainerOrganizer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn viewContainerOrganizer_capacity() {
        let s = ViewContainerOrganizer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn viewContainerOrganizer_search() {
        let mut s = ViewContainerOrganizer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn viewContainerOrganizer_stats() {
        let mut s = ViewContainerOrganizer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn viewVisibilityToggle_new() {
        let m = ViewVisibilityToggle::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn viewVisibilityToggle_add_find() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn viewVisibilityToggle_priority_filter() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("a", "A").with_priority(ViewVisibilityTogglePriority::High));
        m.add(ViewVisibilityToggleItem::new("b", "B").with_priority(ViewVisibilityTogglePriority::Low));
        m.add(ViewVisibilityToggleItem::new("c", "C").with_priority(ViewVisibilityTogglePriority::High));
        assert_eq!(m.by_priority(ViewVisibilityTogglePriority::High).len(), 2);
    }

    #[test]
    fn viewVisibilityToggle_remove() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn viewVisibilityToggle_search() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("id1", "Hello World"));
        m.add(ViewVisibilityToggleItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn viewVisibilityToggle_total_weight() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("a", "A").with_priority(ViewVisibilityTogglePriority::Critical));
        m.add(ViewVisibilityToggleItem::new("b", "B").with_priority(ViewVisibilityTogglePriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn viewVisibilityToggle_capacity_limit() {
        let mut m = ViewVisibilityToggle::new().with_max_items(2);
        m.add(ViewVisibilityToggleItem::new("1", "one"));
        m.add(ViewVisibilityToggleItem::new("2", "two"));
        assert!(!m.add(ViewVisibilityToggleItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn viewVisibilityToggle_sorted_by_priority() {
        let mut m = ViewVisibilityToggle::new();
        m.add(ViewVisibilityToggleItem::new("lo", "Low").with_priority(ViewVisibilityTogglePriority::Low));
        m.add(ViewVisibilityToggleItem::new("hi", "High").with_priority(ViewVisibilityTogglePriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn viewVisibilityToggle_item_metadata() {
        let mut item = ViewVisibilityToggleItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn viewContainerOrganizer_enabled_toggle() {
        let mut s = ViewContainerOrganizer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn viewVisibilityToggle_priority_display() {
        assert_eq!(format!("{}", ViewVisibilityTogglePriority::High), "high");
        assert_eq!(format!("{}", ViewVisibilityTogglePriority::Low), "low");
    }


    #[test]
    fn wb_views_entry_creation() {
        let e = WbViewsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_views_entry_with_priority() {
        let e = WbViewsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_views_entry_metadata() {
        let e = WbViewsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_views_entry_remove_meta() {
        let mut e = WbViewsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_views_entry_activate_deactivate() {
        let mut e = WbViewsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_views_config_add_sorted() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("lo", "Lo").with_priority(1));
        c.add(WbViewsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_views_config_capacity() {
        let mut c = WbViewsConfig::new(1);
        assert!(c.add(WbViewsEntry::new("a", "A")));
        assert!(!c.add(WbViewsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_views_config_remove() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_views_config_get() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_views_config_active_entries() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        c.add(WbViewsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_views_config_enable_disable() {
        let mut c = WbViewsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_views_config_clear() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_views_config_find_by_label() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_views_config_top_n() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A").with_priority(1));
        c.add(WbViewsEntry::new("b", "B").with_priority(2));
        c.add(WbViewsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_views_config_deactivate_activate_all() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        c.add(WbViewsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_views_config_highest_priority() {
        let mut c = WbViewsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbViewsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_views_config_contains() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_views_config_labels() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "Alpha"));
        c.add(WbViewsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_views_config_drain_inactive() {
        let mut c = WbViewsConfig::new(10);
        c.add(WbViewsEntry::new("a", "A"));
        c.add(WbViewsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yy_metrics_empty() {
        let m = YyMetrics::new("wb_views");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yy_metrics_record_and_mean() {
        let mut m = YyMetrics::new("wb_views");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yy_metrics_min_max() {
        let mut m = YyMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yy_metrics_variance_and_std() {
        let mut m = YyMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yy_metrics_percentile() {
        let mut m = YyMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yy_metrics_merge() {
        let mut a = YyMetrics::new("a");
        a.record(1.0);
        let mut b = YyMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yy_metrics_reset() {
        let mut m = YyMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yy_rate_window_empty() {
        let rw = YyRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yy_rate_window_tick_and_rate() {
        let mut rw = YyRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yy_lru_cache_basic() {
        let mut c = YyLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yy_lru_cache_contains_and_keys() {
        let mut c = YyLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yy_lru_cache_remove() {
        let mut c = YyLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yy_metrics_sum() {
        let mut m = YyMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yy_metrics_label() {
        let m = YyMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yy_lru_cache_clear() {
        let mut c = YyLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
