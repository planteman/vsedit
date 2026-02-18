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


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_views
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbViewsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbViewsRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbViewsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbViewsCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbViewsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 233
// ---------------------------------------------------------------------------

/// Generic object pool `Xc233Pool<T>`.
pub struct Xc233Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc233Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc233PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc233Pool<T> {
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
    pub fn stats(&self) -> Xc233PoolStats {
        Xc233PoolStats {
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

impl<T> Default for Xc233Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc233Scheduler`.
pub struct Xc233Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc233Scheduler {
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

impl Default for Xc233Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_233 hash for the given byte slice.
pub fn xc_233_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_233 convention.
pub fn xc_233_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_113 deepening: state machine + event bus ---

/// States for the Xd113 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd113State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd113State {
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
pub struct Xd113Transition {
    pub from: Xd113State,
    pub to: Xd113State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd113StateMachine {
    current: Xd113State,
    history: Vec<Xd113Transition>,
    step_counter: usize,
}

impl Xd113StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd113State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd113State {
        self.current
    }

    pub fn history(&self) -> &[Xd113Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd113State) -> Result<Xd113State, String> {
        let allowed = match (self.current, target) {
            (Xd113State::Idle, Xd113State::Running) => true,
            (Xd113State::Running, Xd113State::Paused) => true,
            (Xd113State::Running, Xd113State::Done) => true,
            (Xd113State::Paused, Xd113State::Running) => true,
            (Xd113State::Paused, Xd113State::Done) => true,
            (Xd113State::Done, Xd113State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_113: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd113Transition {
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
            "Xd113SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd113State> {
        let prefix = "Xd113SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd113State::Idle),
            "Running" => Some(Xd113State::Running),
            "Paused" => Some(Xd113State::Paused),
            "Done" => Some(Xd113State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd113State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd113 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd113Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd113Event {
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

type Xd113HandlerFn = Box<dyn Fn(&Xd113Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd113EventBus {
    handlers: Vec<(usize, Option<String>, Xd113HandlerFn)>,
    next_id: usize,
    published: Vec<Xd113Event>,
}

impl Xd113EventBus {
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
        F: Fn(&Xd113Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd113Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd113Event) {
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

    pub fn published_events(&self) -> &[Xd113Event] {
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
// xg_39: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg39Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg39Graph {
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

impl Default for Xg39Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_39: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg39Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg39Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg39Heap<T>) {
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

impl<T: Ord> Default for Xg39Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 232).
pub struct Xh232SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh232SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 274 as u64,
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

/// A compact bit set supporting boolean operations (variant 232).
pub struct Xh232BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh232BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 232).
pub struct Xi232Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi232Deque<T> {
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
pub struct Xi232Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi232Interval {
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

/// A simple interval tree (variant 232).
pub struct Xi232IntervalTree {
    xi_intervals: Vec<Xi232Interval>,
}

impl Xi232IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi232Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi232Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi232Interval) -> Vec<&Xi232Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi232Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi232Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi232Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi232Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi232Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi232Interval> = Vec::new();
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


    // xa_ extended tests for wb_views
    #[test]
    fn xa_wb_views_ring_new() {
        let rb = super::XaWbViewsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_views_ring_push_len() {
        let mut rb = super::XaWbViewsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_views_ring_wrap() {
        let mut rb = super::XaWbViewsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_views_ring_mean_empty() {
        let rb = super::XaWbViewsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_views_ring_mean_values() {
        let mut rb = super::XaWbViewsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_views_ring_min_max() {
        let mut rb = super::XaWbViewsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_views_ring_iter() {
        let mut rb = super::XaWbViewsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_views_counter_new() {
        let c = super::XaWbViewsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_views_counter_inc() {
        let mut c = super::XaWbViewsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_views_counter_inc_by() {
        let mut c = super::XaWbViewsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_views_counter_reset() {
        let mut c = super::XaWbViewsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_views_counter_clear() {
        let mut c = super::XaWbViewsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_views_counter_default() {
        let c = super::XaWbViewsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 233 ----

    #[test]
    fn xc_233_pool_new_empty() {
        let pool: super::Xc233Pool<i32> = super::Xc233Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_233_pool_release_acquire() {
        let mut pool = super::Xc233Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_233_pool_acquire_empty() {
        let mut pool: super::Xc233Pool<i32> = super::Xc233Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_233_pool_full() {
        let mut pool = super::Xc233Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_233_pool_drain() {
        let mut pool = super::Xc233Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_233_pool_stats() {
        let mut pool = super::Xc233Pool::new(8);
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
    fn xc_233_pool_clear() {
        let mut pool = super::Xc233Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_233_pool_shrink() {
        let mut pool = super::Xc233Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_233_pool_default() {
        let pool: super::Xc233Pool<String> = super::Xc233Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_233_pool_extend() {
        let mut pool = super::Xc233Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_233_pool_retain() {
        let mut pool = super::Xc233Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_233_scheduler_round_robin() {
        let mut sched = super::Xc233Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_233_scheduler_empty() {
        let mut sched = super::Xc233Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_233_scheduler_reset() {
        let mut sched = super::Xc233Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_233_scheduler_add_remove() {
        let mut sched = super::Xc233Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_233_scheduler_targets() {
        let sched = super::Xc233Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_233_hash_empty() {
        assert_eq!(super::xc_233_hash(b""), 5381);
    }

    #[test]
    fn xc_233_hash_data() {
        let h = super::xc_233_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_233_hash(b"hello"), h);
    }

    #[test]
    fn xc_233_reverse_str() {
        assert_eq!(super::xc_233_reverse("abc"), "cba");
        assert_eq!(super::xc_233_reverse(""), "");
    }


    // --- xd_113 deepening tests ---

    #[test]
    fn xd_113_sm_initial_state() {
        let sm = Xd113StateMachine::new();
        assert_eq!(sm.current_state(), Xd113State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_113_sm_valid_idle_to_running() {
        let mut sm = Xd113StateMachine::new();
        assert!(sm.transition(Xd113State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd113State::Running);
    }

    #[test]
    fn xd_113_sm_valid_running_to_paused() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        assert!(sm.transition(Xd113State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd113State::Paused);
    }

    #[test]
    fn xd_113_sm_valid_running_to_done() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        assert!(sm.transition(Xd113State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd113State::Done);
    }

    #[test]
    fn xd_113_sm_valid_paused_to_running() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        sm.transition(Xd113State::Paused).unwrap();
        assert!(sm.transition(Xd113State::Running).is_ok());
    }

    #[test]
    fn xd_113_sm_valid_done_to_idle() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        sm.transition(Xd113State::Done).unwrap();
        assert!(sm.transition(Xd113State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd113State::Idle);
    }

    #[test]
    fn xd_113_sm_invalid_idle_to_done() {
        let mut sm = Xd113StateMachine::new();
        assert!(sm.transition(Xd113State::Done).is_err());
    }

    #[test]
    fn xd_113_sm_invalid_idle_to_paused() {
        let mut sm = Xd113StateMachine::new();
        assert!(sm.transition(Xd113State::Paused).is_err());
    }

    #[test]
    fn xd_113_sm_history_tracking() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        sm.transition(Xd113State::Paused).unwrap();
        sm.transition(Xd113State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd113State::Idle);
        assert_eq!(sm.history()[0].to, Xd113State::Running);
        assert_eq!(sm.history()[1].from, Xd113State::Running);
        assert_eq!(sm.history()[2].to, Xd113State::Done);
    }

    #[test]
    fn xd_113_sm_serialize_deserialize() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd113StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd113State::Running));
    }

    #[test]
    fn xd_113_sm_deserialize_invalid() {
        assert_eq!(Xd113StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_113_sm_reset() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd113State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_113_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd113EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd113Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_113_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd113EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd113Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd113Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_113_bus_unsubscribe() {
        let mut bus = Xd113EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_113_event_kind_and_payload() {
        let e = Xd113Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd113Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_113_bus_clear_history() {
        let mut bus = Xd113EventBus::new();
        bus.publish(Xd113Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_113_sm_step_counter_increments() {
        let mut sm = Xd113StateMachine::new();
        sm.transition(Xd113State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd113State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_39 graph tests ------------------------------------------------

    #[test]
    fn xg_39_graph_empty() {
        let g = super::Xg39Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_39_graph_add_node() {
        let mut g = super::Xg39Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_39_graph_add_edge() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_39_graph_neighbors() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_39_graph_has_path() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_39_graph_self_path() {
        let g = super::Xg39Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_39_graph_topo_sort() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_39_graph_cycle_detect_false() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_39_graph_cycle_detect_true() {
        let mut g = super::Xg39Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_39 heap tests -------------------------------------------------

    #[test]
    fn xg_39_heap_empty() {
        let h: super::Xg39Heap<i32> = super::Xg39Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_39_heap_push_pop() {
        let mut h = super::Xg39Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_39_heap_peek() {
        let mut h = super::Xg39Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_39_heap_drain_sorted() {
        let mut h = super::Xg39Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_39_heap_merge() {
        let mut a = super::Xg39Heap::new();
        let mut b = super::Xg39Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_39_heap_default() {
        let h: super::Xg39Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_39_graph_default() {
        let g: super::Xg39Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh232_skip_insert_contains() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh232_skip_remove() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh232_skip_len() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh232_skip_range_query() {
        let mut sl = super::Xh232SkipList::xh_new(4);
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
    fn xh232_skip_floor_ceiling() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh232_skip_rank() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh232_skip_empty() {
        let sl = super::Xh232SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh232_skip_duplicates() {
        let mut sl = super::Xh232SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh232_bitset_set_test() {
        let mut bs = super::Xh232BitSet::xh_new(256);
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
    fn xh232_bitset_clear_count() {
        let mut bs = super::Xh232BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh232_bitset_and_or_xor() {
        let mut a = super::Xh232BitSet::xh_new(128);
        let mut b = super::Xh232BitSet::xh_new(128);
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
    fn xh232_bitset_iter_ones() {
        let mut bs = super::Xh232BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh232_bitset_first_last() {
        let mut bs = super::Xh232BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh232_bitset_empty() {
        let bs = super::Xh232BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi232_deque_push_pop_back() {
        let mut dq = super::Xi232Deque::xi_new(4);
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
    fn xi232_deque_push_pop_front() {
        let mut dq = super::Xi232Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi232_deque_mixed_ops() {
        let mut dq = super::Xi232Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi232_deque_get_and_split() {
        let mut dq = super::Xi232Deque::xi_new(8);
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
    fn xi232_deque_rotate_left() {
        let mut dq = super::Xi232Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi232_deque_rotate_right() {
        let mut dq = super::Xi232Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi232_deque_grow() {
        let mut dq = super::Xi232Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi232_deque_empty() {
        let dq = super::Xi232Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi232_interval_tree_insert_query() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi232Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi232Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi232_interval_tree_overlap() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi232Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi232Interval::xi_new(12, 20));
        let q = super::Xi232Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi232_interval_tree_remove() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi232Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi232_interval_tree_gaps() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi232Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi232Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi232Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi232Interval::xi_new(8, 10));
    }

    #[test]
    fn xi232_interval_tree_merge() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi232Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi232Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi232Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi232Interval::xi_new(10, 15));
    }

    #[test]
    fn xi232_interval_tree_all() {
        let mut tree = super::Xi232IntervalTree::xi_new();
        tree.xi_insert(super::Xi232Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi232Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi232_interval_tree_empty() {
        let tree = super::Xi232IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi232_interval_tree_contains_point() {
        let iv = super::Xi232Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
