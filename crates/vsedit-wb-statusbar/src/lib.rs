//! Status bar management service.
//!
//! Provides [`StatusBarService`] for managing left- and right-aligned status
//! bar items, each with configurable priority, visibility, and styling.

use vsedit_events::{Emitter, Event};

// ---------------------------------------------------------------------------
// StatusBarAlignment
// ---------------------------------------------------------------------------

/// Alignment of a status bar item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAlignment {
    Left,
    Center,
    Right,
}

// ---------------------------------------------------------------------------
// StatusBarItem
// ---------------------------------------------------------------------------

/// A single item displayed in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub alignment: StatusBarAlignment,
    /// Higher priority items appear further to the respective edge.
    pub priority: i32,
    pub visible: bool,
    pub background_color: Option<String>,
    pub foreground_color: Option<String>,
}

// ---------------------------------------------------------------------------
// StatusBarService
// ---------------------------------------------------------------------------

/// Manages a collection of [`StatusBarItem`]s and notifies listeners on change.
pub struct StatusBarService {
    items: Vec<StatusBarItem>,
    on_did_change: Emitter<()>,
}

impl StatusBarService {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            on_did_change: Emitter::new(),
        }
    }

    /// Add an item and return its id.
    pub fn add_item(&mut self, item: StatusBarItem) -> String {
        let id = item.id.clone();
        self.items.push(item);
        self.on_did_change.fire(&());
        id
    }

    /// Update the text of an existing item.
    pub fn update_item(&mut self, id: &str, text: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.text = text.to_string();
            self.on_did_change.fire(&());
        }
    }

    /// Remove an item by id.
    pub fn remove_item(&mut self, id: &str) {
        let len = self.items.len();
        self.items.retain(|i| i.id != id);
        if self.items.len() != len {
            self.on_did_change.fire(&());
        }
    }

    /// Set the visibility of an item.
    pub fn set_visibility(&mut self, id: &str, visible: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            if item.visible != visible {
                item.visible = visible;
                self.on_did_change.fire(&());
            }
        }
    }

    /// Return visible left-aligned items sorted by priority descending.
    pub fn get_left_items(&self) -> Vec<&StatusBarItem> {
        let mut items: Vec<&StatusBarItem> = self
            .items
            .iter()
            .filter(|i| i.alignment == StatusBarAlignment::Left && i.visible)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items
    }

    /// Return visible right-aligned items sorted by priority descending.
    pub fn get_right_items(&self) -> Vec<&StatusBarItem> {
        let mut items: Vec<&StatusBarItem> = self
            .items
            .iter()
            .filter(|i| i.alignment == StatusBarAlignment::Right && i.visible)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items
    }

    /// Subscribe to change notifications.
    pub fn on_did_change(&self) -> Event<()> {
        self.on_did_change.event()
    }
}

impl Default for StatusBarService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Default items
// ---------------------------------------------------------------------------

/// Register the default set of status bar items.
pub fn register_default_items(svc: &mut StatusBarService) {
    svc.add_item(StatusBarItem {
        id: "statusbar.branch".into(),
        text: String::new(),
        tooltip: Some("Current branch".into()),
        command: None,
        alignment: StatusBarAlignment::Left,
        priority: 100,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.diagnostics".into(),
        text: "✖ 0 ⚠ 0".into(),
        tooltip: Some("Errors and Warnings".into()),
        command: Some("workbench.actions.view.problems".into()),
        alignment: StatusBarAlignment::Left,
        priority: 90,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.lineColumn".into(),
        text: "Ln 1, Col 1".into(),
        tooltip: Some("Go to Line/Column".into()),
        command: Some("editor.action.gotoLine".into()),
        alignment: StatusBarAlignment::Right,
        priority: 100,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.encoding".into(),
        text: "UTF-8".into(),
        tooltip: Some("Select Encoding".into()),
        command: Some("editor.action.changeEncoding".into()),
        alignment: StatusBarAlignment::Right,
        priority: 90,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.eol".into(),
        text: "LF".into(),
        tooltip: Some("Select End of Line Sequence".into()),
        command: Some("editor.action.changeEol".into()),
        alignment: StatusBarAlignment::Right,
        priority: 80,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.language".into(),
        text: "Plain Text".into(),
        tooltip: Some("Select Language Mode".into()),
        command: Some("editor.action.changeLanguageMode".into()),
        alignment: StatusBarAlignment::Right,
        priority: 70,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.indentation".into(),
        text: "Spaces: 4".into(),
        tooltip: Some("Select Indentation".into()),
        command: Some("editor.action.changeIndentation".into()),
        alignment: StatusBarAlignment::Right,
        priority: 60,
        visible: true,
        background_color: None,
        foreground_color: None,
    });

    svc.add_item(StatusBarItem {
        id: "statusbar.notification".into(),
        text: String::new(),
        tooltip: Some("Notifications".into()),
        command: Some("notifications.show".into()),
        alignment: StatusBarAlignment::Right,
        priority: 10,
        visible: true,
        background_color: None,
        foreground_color: None,
    });
}

// ---------------------------------------------------------------------------
// StatusBarItemBuilder
// ---------------------------------------------------------------------------

/// A builder for constructing [`StatusBarItem`] instances using the builder
/// pattern. Provides sensible defaults and a fluent API.
pub struct StatusBarItemBuilder {
    id: String,
    text: String,
    tooltip: Option<String>,
    command: Option<String>,
    alignment: StatusBarAlignment,
    priority: i32,
    visible: bool,
    background_color: Option<String>,
    foreground_color: Option<String>,
}

impl StatusBarItemBuilder {
    /// Create a new builder with the given `id` and sensible defaults.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: String::new(),
            tooltip: None,
            command: None,
            alignment: StatusBarAlignment::Left,
            priority: 0,
            visible: true,
            background_color: None,
            foreground_color: None,
        }
    }

    /// Set the display text.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Set the tooltip.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set the command to execute when clicked.
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Set the alignment (Left or Right).
    pub fn alignment(mut self, alignment: StatusBarAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the priority (higher = closer to the edge).
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set the background color.
    pub fn background_color(mut self, color: impl Into<String>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    /// Set the foreground color.
    pub fn foreground_color(mut self, color: impl Into<String>) -> Self {
        self.foreground_color = Some(color.into());
        self
    }

    /// Consume the builder and produce a [`StatusBarItem`].
    pub fn build(self) -> StatusBarItem {
        StatusBarItem {
            id: self.id,
            text: self.text,
            tooltip: self.tooltip,
            command: self.command,
            alignment: self.alignment,
            priority: self.priority,
            visible: self.visible,
            background_color: self.background_color,
            foreground_color: self.foreground_color,
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

use std::fmt;

impl fmt::Display for StatusBarAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarAlignment::Left => write!(f, "Left"),
            StatusBarAlignment::Center => write!(f, "Center"),
            StatusBarAlignment::Right => write!(f, "Right"),
        }
    }
}

impl fmt::Display for StatusBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.text, self.alignment)
    }
}

impl StatusBarItem {
    /// Returns `true` if this item has a command associated with it.
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }

    /// Returns `true` if this item has a tooltip.
    pub fn has_tooltip(&self) -> bool {
        self.tooltip.is_some()
    }
}

// ---------------------------------------------------------------------------
// Additional StatusBarService helpers
// ---------------------------------------------------------------------------

impl StatusBarService {
    /// Return the total number of items (visible or not).
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Find an item by its `id`.
    pub fn get_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Find an item mutably by its `id`.
    pub fn get_item_mut(&mut self, id: &str) -> Option<&mut StatusBarItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Update the tooltip of an item. Fires the change event.
    pub fn update_tooltip(&mut self, id: &str, tooltip: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.tooltip = Some(tooltip.to_string());
            self.on_did_change.fire(&());
        }
    }

    /// Update the background and/or foreground colors of an item.
    /// Pass `None` to leave a color unchanged.
    pub fn update_colors(&mut self, id: &str, bg: Option<&str>, fg: Option<&str>) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            if let Some(bg) = bg {
                item.background_color = Some(bg.to_string());
            }
            if let Some(fg) = fg {
                item.foreground_color = Some(fg.to_string());
            }
            self.on_did_change.fire(&());
        }
    }

    /// Return the number of currently visible items.
    pub fn visible_count(&self) -> usize {
        self.items.iter().filter(|i| i.visible).count()
    }

    /// Returns `true` if the service has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Find all items whose text contains the given substring.
    pub fn find_by_text(&self, text: &str) -> Vec<&StatusBarItem> {
        self.items.iter().filter(|i| i.text.contains(text)).collect()
    }

    /// Remove all items from the service.
    pub fn clear_all(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.on_did_change.fire(&());
        }
    }

    /// Return a slice of all items.
    pub fn get_all_items(&self) -> &[StatusBarItem] {
        &self.items
    }
}

// ---------------------------------------------------------------------------
// Sort items by priority
// ---------------------------------------------------------------------------

/// Sort a mutable slice of status bar items by priority (highest first).
pub fn sort_items_by_priority(items: &mut [StatusBarItem]) {
    items.sort_by(|a, b| b.priority.cmp(&a.priority));
}

// ---------------------------------------------------------------------------
// Item group management
// ---------------------------------------------------------------------------

/// A named group of status bar item IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarGroup {
    pub name: String,
    pub item_ids: Vec<String>,
}

impl StatusBarGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            item_ids: Vec::new(),
        }
    }

    pub fn add(&mut self, id: impl Into<String>) {
        self.item_ids.push(id.into());
    }

    pub fn remove(&mut self, id: &str) {
        self.item_ids.retain(|i| i != id);
    }

    pub fn contains(&self, id: &str) -> bool {
        self.item_ids.iter().any(|i| i == id)
    }

    pub fn len(&self) -> usize {
        self.item_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.item_ids.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Status bar width computation
// ---------------------------------------------------------------------------

/// Compute the total display width needed for a slice of items.
///
/// Each item contributes its text length plus a separator of `sep_width` chars
/// (no trailing separator).
pub fn compute_status_bar_width(items: &[StatusBarItem], sep_width: usize) -> usize {
    if items.is_empty() {
        return 0;
    }
    let text_width: usize = items.iter().map(|i| i.text.len()).sum();
    let separators = (items.len() - 1) * sep_width;
    text_width + separators
}

// ---------------------------------------------------------------------------
// Item animation state
// ---------------------------------------------------------------------------

/// Animation state for a status bar item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPhase {
    Idle,
    FadingIn,
    Visible,
    FadingOut,
}

/// Tracks animation state for a status bar item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAnimationState {
    pub item_id: String,
    pub phase: AnimationPhase,
    pub elapsed_ms: u64,
    pub duration_ms: u64,
}

impl ItemAnimationState {
    pub fn new(item_id: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            item_id: item_id.into(),
            phase: AnimationPhase::Idle,
            elapsed_ms: 0,
            duration_ms,
        }
    }

    /// Returns the progress ratio (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).min(1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }
}

impl StatusBarService {
    /// Returns the number of items that are currently hidden.
    pub fn hidden_count(&self) -> usize {
        self.items.iter().filter(|i| !i.visible).count()
    }
}

// ---------------------------------------------------------------------------
// StatusBarSection
// ---------------------------------------------------------------------------

/// Logical grouping of status bar items within a named section.
#[derive(Debug, Clone)]
pub struct StatusBarSection {
    pub name: String,
    pub alignment: StatusBarAlignment,
    pub items: Vec<StatusBarItem>,
}

impl StatusBarSection {
    pub fn new(name: impl Into<String>, alignment: StatusBarAlignment) -> Self {
        Self {
            name: name.into(),
            alignment,
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item: StatusBarItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }

    /// Returns items sorted by priority descending.
    pub fn get_items_sorted(&self) -> Vec<&StatusBarItem> {
        let mut sorted: Vec<&StatusBarItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }
}

// ---------------------------------------------------------------------------
// StatusBarLayout
// ---------------------------------------------------------------------------

/// Manages left, center, and right status bar sections.
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    pub left: StatusBarSection,
    pub center: StatusBarSection,
    pub right: StatusBarSection,
}

impl StatusBarLayout {
    pub fn new() -> Self {
        Self {
            left: StatusBarSection::new("left", StatusBarAlignment::Left),
            center: StatusBarSection::new("center", StatusBarAlignment::Center),
            right: StatusBarSection::new("right", StatusBarAlignment::Right),
        }
    }

    pub fn add_to_left(&mut self, item: StatusBarItem) {
        self.left.add_item(item);
    }

    pub fn add_to_center(&mut self, item: StatusBarItem) {
        self.center.add_item(item);
    }

    pub fn add_to_right(&mut self, item: StatusBarItem) {
        self.right.add_item(item);
    }

    pub fn get_left(&self) -> Vec<&StatusBarItem> {
        self.left.get_items_sorted()
    }

    pub fn get_center(&self) -> Vec<&StatusBarItem> {
        self.center.get_items_sorted()
    }

    pub fn get_right(&self) -> Vec<&StatusBarItem> {
        self.right.get_items_sorted()
    }

    pub fn total_items(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Removes the item with the given id from whichever section contains it.
    pub fn remove_item(&mut self, id: &str) {
        self.left.remove_item(id);
        self.center.remove_item(id);
        self.right.remove_item(id);
    }
}

// ---------------------------------------------------------------------------
// Animation helpers
// ---------------------------------------------------------------------------

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const DOTS_FRAMES: &[&str] = &[".", "..", "..."];

/// Returns the appropriate frame for the given elapsed time.
///
/// If `frames` is empty the function returns `""`.
pub fn animation_frame<'a>(elapsed_ms: u64, frames: &[&'a str]) -> &'a str {
    if frames.is_empty() {
        return "";
    }
    let idx = (elapsed_ms / 80) as usize % frames.len();
    frames[idx]
}

// ---------------------------------------------------------------------------
// StatusBarNotification — temporary notification system
// ---------------------------------------------------------------------------

/// Priority level for status bar notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

/// A temporary notification displayed in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarNotification {
    pub id: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub duration_ms: u64,
    pub elapsed_ms: u64,
    pub icon: Option<String>,
    pub dismissed: bool,
}

impl StatusBarNotification {
    pub fn new(id: impl Into<String>, message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            priority: NotificationPriority::Normal,
            duration_ms,
            elapsed_ms: 0,
            icon: None,
            dismissed: false,
        }
    }

    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Advance the elapsed time. Returns true if the notification has expired.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        self.is_expired()
    }

    pub fn is_expired(&self) -> bool {
        self.elapsed_ms >= self.duration_ms || self.dismissed
    }

    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    /// Display text including optional icon prefix.
    pub fn display_text(&self) -> String {
        match &self.icon {
            Some(icon) => format!("{} {}", icon, self.message),
            None => self.message.clone(),
        }
    }

    /// Remaining time in milliseconds.
    pub fn remaining_ms(&self) -> u64 {
        self.duration_ms.saturating_sub(self.elapsed_ms)
    }
}

/// Manages a queue of status bar notifications.
#[derive(Debug, Clone, Default)]
pub struct NotificationQueue {
    notifications: Vec<StatusBarNotification>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, notification: StatusBarNotification) {
        self.notifications.push(notification);
        self.notifications.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove expired notifications, returning the count removed.
    pub fn sweep(&mut self) -> usize {
        let before = self.notifications.len();
        self.notifications.retain(|n| !n.is_expired());
        before - self.notifications.len()
    }

    /// Tick all notifications and sweep expired ones.
    pub fn tick(&mut self, delta_ms: u64) -> usize {
        for n in &mut self.notifications {
            n.tick(delta_ms);
        }
        self.sweep()
    }

    /// The highest-priority active notification.
    pub fn current(&self) -> Option<&StatusBarNotification> {
        self.notifications.first()
    }

    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.dismiss();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}

// ---------------------------------------------------------------------------
// StatusBarProgressItem — progress indicator
// ---------------------------------------------------------------------------

/// A progress indicator displayed in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarProgressItem {
    pub id: String,
    pub label: String,
    pub current: u64,
    pub total: u64,
    pub completed: bool,
}

impl StatusBarProgressItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, total: u64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            current: 0,
            total,
            completed: false,
        }
    }

    /// Advance by `amount`. Marks completed when current >= total.
    pub fn advance(&mut self, amount: u64) {
        self.current = self.current.saturating_add(amount).min(self.total);
        if self.current >= self.total {
            self.completed = true;
        }
    }

    /// Progress ratio in [0.0, 1.0].
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.current as f64 / self.total as f64
    }

    /// Percentage as an integer (0–100).
    pub fn percentage(&self) -> u8 {
        (self.ratio() * 100.0).round() as u8
    }

    /// A display string like "Building: 45%".
    pub fn display_text(&self) -> String {
        format!("{}: {}%", self.label, self.percentage())
    }
}

// ---------------------------------------------------------------------------
// StatusBarContextMenu
// ---------------------------------------------------------------------------

/// An action within a status bar context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuAction {
    pub label: String,
    pub command: String,
    pub enabled: bool,
}

impl ContextMenuAction {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// A context menu attached to a status bar item.
#[derive(Debug, Clone)]
pub struct StatusBarContextMenu {
    pub item_id: String,
    pub actions: Vec<ContextMenuAction>,
    pub selected: usize,
    pub visible: bool,
}

impl StatusBarContextMenu {
    pub fn new(item_id: impl Into<String>) -> Self {
        Self {
            item_id: item_id.into(),
            actions: Vec::new(),
            selected: 0,
            visible: false,
        }
    }

    pub fn add_action(&mut self, action: ContextMenuAction) {
        self.actions.push(action);
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.selected = 0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn select_next(&mut self) {
        if !self.actions.is_empty() {
            self.selected = (self.selected + 1) % self.actions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.actions.is_empty() {
            self.selected = if self.selected == 0 {
                self.actions.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Returns the selected action's command if it is enabled.
    pub fn activate(&self) -> Option<&str> {
        self.actions.get(self.selected).and_then(|a| {
            if a.enabled {
                Some(a.command.as_str())
            } else {
                None
            }
        })
    }

    /// Number of enabled actions.
    pub fn enabled_count(&self) -> usize {
        self.actions.iter().filter(|a| a.enabled).count()
    }
}

// ---------------------------------------------------------------------------
// StatusBarLayout — responsive layout calculations
// ---------------------------------------------------------------------------

impl StatusBarLayout {
    /// Compute responsive widths for (left, center, right) given a total width.
    /// Center is clamped to not overlap left/right. Returns (left_w, center_w, right_w).
    pub fn compute_responsive_widths(&self, total_width: usize, sep: usize) -> (usize, usize, usize) {
        let left_w = compute_status_bar_width(
            &self.left.items.iter().filter(|i| i.visible).cloned().collect::<Vec<_>>(),
            sep,
        );
        let right_w = compute_status_bar_width(
            &self.right.items.iter().filter(|i| i.visible).cloned().collect::<Vec<_>>(),
            sep,
        );
        let center_w = compute_status_bar_width(
            &self.center.items.iter().filter(|i| i.visible).cloned().collect::<Vec<_>>(),
            sep,
        );
        let available_center = total_width.saturating_sub(left_w + right_w);
        let clamped_center = center_w.min(available_center);
        (left_w, clamped_center, right_w)
    }

    /// Returns true if items overflow the given width.
    pub fn overflows(&self, total_width: usize, sep: usize) -> bool {
        let (l, c, r) = self.compute_responsive_widths(total_width, sep);
        l + c + r > total_width
    }

    /// Find an item by id across all sections.
    pub fn find_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.left.get_item(id)
            .or_else(|| self.center.get_item(id))
            .or_else(|| self.right.get_item(id))
    }

    /// Count of all visible items across sections.
    pub fn visible_count(&self) -> usize {
        self.left.items.iter().filter(|i| i.visible).count()
            + self.center.items.iter().filter(|i| i.visible).count()
            + self.right.items.iter().filter(|i| i.visible).count()
    }
}

impl StatusBarItem {
    /// Return a compact summary string: "id: text (alignment, priority)".
    pub fn summary(&self) -> String {
        format!("{}: {} ({}, p={})", self.id, self.text, self.alignment, self.priority)
    }

    /// Return the display width (character count) of this item's text.
    pub fn text_width(&self) -> usize {
        self.text.len()
    }

    /// Return true if this item has custom colors set.
    pub fn has_custom_colors(&self) -> bool {
        self.background_color.is_some() || self.foreground_color.is_some()
    }
}

impl StatusBarService {
    /// Return items sorted by priority across all alignments (highest first).
    pub fn items_sorted_by_priority(&self) -> Vec<&StatusBarItem> {
        let mut items: Vec<&StatusBarItem> = self.items.iter().collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items
    }

    /// Return the sum of all text widths for visible items.
    pub fn total_visible_text_width(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.visible)
            .map(|i| i.text.len())
            .sum()
    }

    /// Return distinct alignment types present among visible items.
    pub fn active_alignments(&self) -> Vec<StatusBarAlignment> {
        let mut aligns = Vec::new();
        for &a in &[StatusBarAlignment::Left, StatusBarAlignment::Center, StatusBarAlignment::Right] {
            if self.items.iter().any(|i| i.visible && i.alignment == a) {
                aligns.push(a);
            }
        }
        aligns
    }
}

impl StatusBarSection {
    /// Return the total text width of visible items in this section.
    pub fn total_text_width(&self) -> usize {
        self.items.iter().filter(|i| i.visible).map(|i| i.text.len()).sum()
    }

    /// Return the number of visible items in this section.
    pub fn visible_count(&self) -> usize {
        self.items.iter().filter(|i| i.visible).count()
    }
}

impl StatusBarGroup {
    /// Return the item IDs as a comma-separated string.
    pub fn ids_display(&self) -> String {
        self.item_ids.join(", ")
    }
}

impl StatusBarNotification {
    /// Return the remaining time as a percentage (0.0–100.0).
    pub fn remaining_pct(&self) -> f64 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        (self.remaining_ms() as f64 / self.duration_ms as f64) * 100.0
    }
}

// ---------------------------------------------------------------------------
// StatusBarLanguageSelector
// ---------------------------------------------------------------------------

/// Manages the language mode selector in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarLanguageSelector {
    pub current_language: String,
    pub available_languages: Vec<String>,
}

impl StatusBarLanguageSelector {
    pub fn new(current: impl Into<String>) -> Self {
        Self {
            current_language: current.into(),
            available_languages: Vec::new(),
        }
    }

    /// Set available languages for the picker.
    pub fn set_available(&mut self, languages: Vec<String>) {
        self.available_languages = languages;
        self.available_languages.sort();
    }

    /// Select a language by name. Returns true if it was in the available list.
    pub fn select(&mut self, language: &str) -> bool {
        if self.available_languages.contains(&language.to_string()) || self.available_languages.is_empty() {
            self.current_language = language.to_string();
            true
        } else {
            false
        }
    }

    /// Filter available languages by prefix.
    pub fn filter(&self, prefix: &str) -> Vec<&str> {
        let lower = prefix.to_lowercase();
        self.available_languages.iter()
            .filter(|l| l.to_lowercase().starts_with(&lower))
            .map(|s| s.as_str())
            .collect()
    }

    /// Build a status bar item for this selector.
    pub fn to_status_item(&self, id: &str) -> StatusBarItem {
        StatusBarItemBuilder::new(id)
            .text(&self.current_language)
            .tooltip("Select Language Mode")
            .command("workbench.action.editor.changeLanguageMode")
            .alignment(StatusBarAlignment::Right)
            .priority(100)
            .build()
    }
}

impl std::fmt::Display for StatusBarLanguageSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Language: {}", self.current_language)
    }
}

// ---------------------------------------------------------------------------
// StatusBarEncodingSelector
// ---------------------------------------------------------------------------

/// Manages the file encoding selector in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarEncodingSelector {
    pub current_encoding: String,
    pub available_encodings: Vec<String>,
}

impl StatusBarEncodingSelector {
    pub fn new(encoding: impl Into<String>) -> Self {
        Self {
            current_encoding: encoding.into(),
            available_encodings: vec![
                "UTF-8".into(), "UTF-16 LE".into(), "UTF-16 BE".into(),
                "ASCII".into(), "ISO-8859-1".into(), "Windows-1252".into(),
            ],
        }
    }

    pub fn select(&mut self, encoding: &str) -> bool {
        if self.available_encodings.iter().any(|e| e == encoding) {
            self.current_encoding = encoding.to_string();
            true
        } else {
            false
        }
    }

    pub fn to_status_item(&self, id: &str) -> StatusBarItem {
        StatusBarItemBuilder::new(id)
            .text(&self.current_encoding)
            .tooltip("Select Encoding")
            .command("workbench.action.editor.changeEncoding")
            .alignment(StatusBarAlignment::Right)
            .priority(90)
            .build()
    }
}

impl std::fmt::Display for StatusBarEncodingSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Encoding: {}", self.current_encoding)
    }
}

// ---------------------------------------------------------------------------
// StatusBarLineEndingSelector
// ---------------------------------------------------------------------------

/// Line ending mode for the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    LF,
    CRLF,
}

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LF => write!(f, "LF"),
            Self::CRLF => write!(f, "CRLF"),
        }
    }
}

/// Manages the line ending selector in the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarLineEndingSelector {
    pub current: LineEnding,
}

impl StatusBarLineEndingSelector {
    pub fn new(ending: LineEnding) -> Self {
        Self { current: ending }
    }

    pub fn toggle(&mut self) -> LineEnding {
        self.current = match self.current {
            LineEnding::LF => LineEnding::CRLF,
            LineEnding::CRLF => LineEnding::LF,
        };
        self.current
    }

    pub fn set(&mut self, ending: LineEnding) {
        self.current = ending;
    }

    pub fn to_status_item(&self, id: &str) -> StatusBarItem {
        StatusBarItemBuilder::new(id)
            .text(&format!("{}", self.current))
            .tooltip("Select End of Line Sequence")
            .command("workbench.action.editor.changeEOL")
            .alignment(StatusBarAlignment::Right)
            .priority(80)
            .build()
    }
}

impl std::fmt::Display for StatusBarLineEndingSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EOL: {}", self.current)
    }
}

// ---------------------------------------------------------------------------
// Status bar click action dispatcher
// ---------------------------------------------------------------------------

/// Action triggered when a status bar item is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarClickAction {
    pub item_id: String,
    pub command: String,
    pub args: Vec<String>,
}

impl StatusBarClickAction {
    pub fn new(item_id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            item_id: item_id.into(),
            command: command.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

impl std::fmt::Display for StatusBarClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClickAction({} -> {})", self.item_id, self.command)
    }
}

/// Dispatcher that resolves click actions for status bar items.
#[derive(Debug, Clone)]
pub struct StatusBarActionDispatcher {
    actions: Vec<StatusBarClickAction>,
}

impl StatusBarActionDispatcher {
    pub fn new() -> Self {
        Self { actions: Vec::new() }
    }

    pub fn register(&mut self, action: StatusBarClickAction) {
        // Replace existing action for same item_id
        self.actions.retain(|a| a.item_id != action.item_id);
        self.actions.push(action);
    }

    /// Dispatch a click on the given item_id, returning the action if found.
    pub fn dispatch(&self, item_id: &str) -> Option<&StatusBarClickAction> {
        self.actions.iter().find(|a| a.item_id == item_id)
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn has_action(&self, item_id: &str) -> bool {
        self.actions.iter().any(|a| a.item_id == item_id)
    }
}

impl Default for StatusBarActionDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StatusBarActionDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StatusBarActionDispatcher({} actions)", self.actions.len())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_item(id: &str, alignment: StatusBarAlignment, priority: i32) -> StatusBarItem {
        StatusBarItem {
            id: id.into(),
            text: id.into(),
            tooltip: None,
            command: None,
            alignment,
            priority,
            visible: true,
            background_color: None,
            foreground_color: None,
        }
    }

    #[test]
    fn add_and_remove_items() {
        let mut svc = StatusBarService::new();
        let id = svc.add_item(make_item("a", StatusBarAlignment::Left, 10));
        assert_eq!(id, "a");
        assert_eq!(svc.get_left_items().len(), 1);

        svc.remove_item("a");
        assert!(svc.get_left_items().is_empty());
    }

    #[test]
    fn update_item_text() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("x", StatusBarAlignment::Left, 10));
        svc.update_item("x", "updated");
        assert_eq!(svc.get_left_items()[0].text, "updated");
    }

    #[test]
    fn left_items_sorted_by_priority_desc() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("low", StatusBarAlignment::Left, 10));
        svc.add_item(make_item("high", StatusBarAlignment::Left, 50));
        svc.add_item(make_item("mid", StatusBarAlignment::Left, 30));

        let items = svc.get_left_items();
        assert_eq!(items[0].id, "high");
        assert_eq!(items[1].id, "mid");
        assert_eq!(items[2].id, "low");
    }

    #[test]
    fn right_items_sorted_by_priority_desc() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("a", StatusBarAlignment::Right, 5));
        svc.add_item(make_item("b", StatusBarAlignment::Right, 20));

        let items = svc.get_right_items();
        assert_eq!(items[0].id, "b");
        assert_eq!(items[1].id, "a");
    }

    #[test]
    fn alignment_filtering() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("l", StatusBarAlignment::Left, 10));
        svc.add_item(make_item("r", StatusBarAlignment::Right, 10));

        assert_eq!(svc.get_left_items().len(), 1);
        assert_eq!(svc.get_left_items()[0].id, "l");
        assert_eq!(svc.get_right_items().len(), 1);
        assert_eq!(svc.get_right_items()[0].id, "r");
    }

    #[test]
    fn visibility_toggle() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("v", StatusBarAlignment::Left, 10));
        assert_eq!(svc.get_left_items().len(), 1);

        svc.set_visibility("v", false);
        assert!(svc.get_left_items().is_empty());

        svc.set_visibility("v", true);
        assert_eq!(svc.get_left_items().len(), 1);
    }

    #[test]
    fn on_did_change_fires() {
        let mut svc = StatusBarService::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _h = svc.on_did_change().on(move |_: &()| {
            *c.lock().unwrap() += 1;
        });

        svc.add_item(make_item("a", StatusBarAlignment::Left, 10));
        svc.update_item("a", "new");
        svc.set_visibility("a", false);
        svc.remove_item("a");

        assert_eq!(*count.lock().unwrap(), 4);
    }

    #[test]
    fn register_default_items_creates_expected() {
        let mut svc = StatusBarService::new();
        register_default_items(&mut svc);

        let left = svc.get_left_items();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].id, "statusbar.branch");
        assert_eq!(left[1].id, "statusbar.diagnostics");

        let right = svc.get_right_items();
        assert_eq!(right.len(), 6);
        // Sorted by priority desc: 100, 90, 80, 70, 60, 10
        assert_eq!(right[0].id, "statusbar.lineColumn");
        assert_eq!(right[1].id, "statusbar.encoding");
        assert_eq!(right[2].id, "statusbar.eol");
        assert_eq!(right[3].id, "statusbar.language");
        assert_eq!(right[4].id, "statusbar.indentation");
        assert_eq!(right[5].id, "statusbar.notification");
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut svc = StatusBarService::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _h = svc.on_did_change().on(move |_: &()| {
            *c.lock().unwrap() += 1;
        });

        svc.remove_item("does-not-exist");
        assert_eq!(*count.lock().unwrap(), 0);
    }

    // -----------------------------------------------------------------------
    // Builder tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_builder_defaults() {
        let item = StatusBarItemBuilder::new("test.id").build();
        assert_eq!(item.id, "test.id");
        assert_eq!(item.text, "");
        assert!(item.tooltip.is_none());
        assert!(item.command.is_none());
        assert_eq!(item.alignment, StatusBarAlignment::Left);
        assert_eq!(item.priority, 0);
        assert!(item.visible);
        assert!(item.background_color.is_none());
        assert!(item.foreground_color.is_none());
    }

    #[test]
    fn test_builder_full_chain() {
        let item = StatusBarItemBuilder::new("full")
            .text("Hello")
            .tooltip("A tooltip")
            .command("do.something")
            .alignment(StatusBarAlignment::Right)
            .priority(42)
            .visible(false)
            .background_color("#ff0000")
            .foreground_color("#00ff00")
            .build();

        assert_eq!(item.id, "full");
        assert_eq!(item.text, "Hello");
        assert_eq!(item.tooltip.as_deref(), Some("A tooltip"));
        assert_eq!(item.command.as_deref(), Some("do.something"));
        assert_eq!(item.alignment, StatusBarAlignment::Right);
        assert_eq!(item.priority, 42);
        assert!(!item.visible);
        assert_eq!(item.background_color.as_deref(), Some("#ff0000"));
        assert_eq!(item.foreground_color.as_deref(), Some("#00ff00"));
    }

    // -----------------------------------------------------------------------
    // Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_statusbar_item_display() {
        let item = StatusBarItemBuilder::new("sb.test")
            .text("branch: main")
            .priority(10)
            .build();
        let display = format!("{}", item);
        assert_eq!(display, "branch: main [Left]");
    }

    #[test]
    fn test_statusbar_alignment_display() {
        assert_eq!(format!("{}", StatusBarAlignment::Left), "Left");
        assert_eq!(format!("{}", StatusBarAlignment::Right), "Right");
    }

    // -----------------------------------------------------------------------
    // Additional StatusBarService method tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_item_count() {
        let mut svc = StatusBarService::new();
        assert_eq!(svc.item_count(), 0);
        svc.add_item(make_item("a", StatusBarAlignment::Left, 1));
        svc.add_item(make_item("b", StatusBarAlignment::Right, 2));
        assert_eq!(svc.item_count(), 2);
        svc.remove_item("a");
        assert_eq!(svc.item_count(), 1);
    }

    #[test]
    fn test_get_item() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("find_me", StatusBarAlignment::Left, 5));
        svc.add_item(make_item("other", StatusBarAlignment::Right, 3));

        let found = svc.get_item("find_me");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "find_me");
        assert_eq!(found.unwrap().priority, 5);

        assert!(svc.get_item("nonexistent").is_none());
    }

    #[test]
    fn test_get_item_mut() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("mut_me", StatusBarAlignment::Left, 1));

        if let Some(item) = svc.get_item_mut("mut_me") {
            item.text = "mutated".to_string();
        }

        assert_eq!(svc.get_item("mut_me").unwrap().text, "mutated");
        assert!(svc.get_item_mut("missing").is_none());
    }

    #[test]
    fn test_update_tooltip() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("tt", StatusBarAlignment::Left, 1));
        assert!(svc.get_item("tt").unwrap().tooltip.is_none());

        svc.update_tooltip("tt", "new tooltip");
        assert_eq!(
            svc.get_item("tt").unwrap().tooltip.as_deref(),
            Some("new tooltip")
        );
    }

    #[test]
    fn test_update_colors() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("col", StatusBarAlignment::Left, 1));

        svc.update_colors("col", Some("#000"), None);
        let item = svc.get_item("col").unwrap();
        assert_eq!(item.background_color.as_deref(), Some("#000"));
        assert!(item.foreground_color.is_none());

        svc.update_colors("col", None, Some("#fff"));
        let item = svc.get_item("col").unwrap();
        assert_eq!(item.background_color.as_deref(), Some("#000"));
        assert_eq!(item.foreground_color.as_deref(), Some("#fff"));
    }

    #[test]
    fn test_visible_count() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("v1", StatusBarAlignment::Left, 1));
        svc.add_item(make_item("v2", StatusBarAlignment::Left, 2));
        svc.add_item(make_item("v3", StatusBarAlignment::Right, 3));
        assert_eq!(svc.visible_count(), 3);

        svc.set_visibility("v2", false);
        assert_eq!(svc.visible_count(), 2);

        svc.set_visibility("v1", false);
        assert_eq!(svc.visible_count(), 1);
    }

    #[test]
    fn test_get_all_items() {
        let mut svc = StatusBarService::new();
        assert!(svc.get_all_items().is_empty());

        svc.add_item(make_item("i1", StatusBarAlignment::Left, 10));
        svc.add_item(make_item("i2", StatusBarAlignment::Right, 20));
        svc.add_item(make_item("i3", StatusBarAlignment::Left, 30));

        let all = svc.get_all_items();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, "i1");
        assert_eq!(all[1].id, "i2");
        assert_eq!(all[2].id, "i3");
    }

    #[test]
    fn test_builder_produces_valid_item_in_service() {
        let mut svc = StatusBarService::new();
        let item = StatusBarItemBuilder::new("builder.item")
            .text("Built")
            .alignment(StatusBarAlignment::Right)
            .priority(55)
            .tooltip("Built via builder")
            .command("builder.cmd")
            .build();

        let id = svc.add_item(item);
        assert_eq!(id, "builder.item");
        assert_eq!(svc.item_count(), 1);

        let found = svc.get_item("builder.item").unwrap();
        assert_eq!(found.text, "Built");
        assert_eq!(found.alignment, StatusBarAlignment::Right);
        assert_eq!(found.priority, 55);
        assert_eq!(found.tooltip.as_deref(), Some("Built via builder"));
        assert_eq!(found.command.as_deref(), Some("builder.cmd"));

        let right = svc.get_right_items();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].id, "builder.item");
    }

    #[test]
    fn test_sort_items_by_priority() {
        let mut items = vec![
            make_item("low", StatusBarAlignment::Left, 10),
            make_item("high", StatusBarAlignment::Left, 100),
            make_item("mid", StatusBarAlignment::Left, 50),
        ];
        sort_items_by_priority(&mut items);
        assert_eq!(items[0].id, "high");
        assert_eq!(items[1].id, "mid");
        assert_eq!(items[2].id, "low");
    }

    #[test]
    fn test_status_bar_group() {
        let mut group = StatusBarGroup::new("editor");
        assert!(group.is_empty());
        group.add("line");
        group.add("col");
        assert_eq!(group.len(), 2);
        assert!(group.contains("line"));
        assert!(!group.contains("missing"));
        group.remove("line");
        assert_eq!(group.len(), 1);
        assert!(!group.contains("line"));
    }

    #[test]
    fn test_compute_status_bar_width() {
        let items = vec![
            make_item("a", StatusBarAlignment::Left, 1),
            make_item("bb", StatusBarAlignment::Left, 2),
            make_item("ccc", StatusBarAlignment::Left, 3),
        ];
        // texts: "a"(1) + "bb"(2) + "ccc"(3) = 6, seps: 2*3 = 6 → 12
        assert_eq!(compute_status_bar_width(&items, 3), 12);
    }

    #[test]
    fn test_compute_status_bar_width_empty() {
        assert_eq!(compute_status_bar_width(&[], 3), 0);
    }

    #[test]
    fn test_animation_state_progress() {
        let mut anim = ItemAnimationState::new("item1", 200);
        assert_eq!(anim.phase, AnimationPhase::Idle);
        assert!((anim.progress() - 0.0).abs() < f64::EPSILON);
        assert!(!anim.is_complete());

        anim.elapsed_ms = 100;
        assert!((anim.progress() - 0.5).abs() < f64::EPSILON);

        anim.elapsed_ms = 300;
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
        assert!(anim.is_complete());
    }

    #[test]
    fn test_animation_state_zero_duration() {
        let anim = ItemAnimationState::new("fast", 0);
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
        assert!(anim.is_complete());
    }

    #[test]
    fn hidden_count_filters_visible() {
        let mut svc = StatusBarService::new();
        let mut a = make_item("a", StatusBarAlignment::Left, 1);
        a.visible = true;
        let mut b = make_item("b", StatusBarAlignment::Right, 2);
        b.visible = false;
        svc.add_item(a);
        svc.add_item(b);
        assert_eq!(svc.hidden_count(), 1);
    }

    // -----------------------------------------------------------------------
    // StatusBarSection tests
    // -----------------------------------------------------------------------

    #[test]
    fn section_add_and_get_items_sorted() {
        let mut sec = StatusBarSection::new("left", StatusBarAlignment::Left);
        sec.add_item(make_item("low", StatusBarAlignment::Left, 10));
        sec.add_item(make_item("high", StatusBarAlignment::Left, 50));
        sec.add_item(make_item("mid", StatusBarAlignment::Left, 30));

        let sorted = sec.get_items_sorted();
        assert_eq!(sorted[0].id, "high");
        assert_eq!(sorted[1].id, "mid");
        assert_eq!(sorted[2].id, "low");
    }

    #[test]
    fn section_remove_item() {
        let mut sec = StatusBarSection::new("left", StatusBarAlignment::Left);
        sec.add_item(make_item("a", StatusBarAlignment::Left, 1));
        sec.add_item(make_item("b", StatusBarAlignment::Left, 2));
        sec.remove_item("a");
        assert_eq!(sec.len(), 1);
        assert!(sec.get_item("a").is_none());
        assert!(sec.get_item("b").is_some());
    }

    #[test]
    fn section_is_empty_and_len() {
        let mut sec = StatusBarSection::new("test", StatusBarAlignment::Left);
        assert!(sec.is_empty());
        assert_eq!(sec.len(), 0);
        sec.add_item(make_item("x", StatusBarAlignment::Left, 1));
        assert!(!sec.is_empty());
        assert_eq!(sec.len(), 1);
    }

    // -----------------------------------------------------------------------
    // StatusBarLayout tests
    // -----------------------------------------------------------------------

    #[test]
    fn layout_add_to_sections_and_get_sorted() {
        let mut layout = StatusBarLayout::new();
        layout.add_to_left(make_item("l1", StatusBarAlignment::Left, 10));
        layout.add_to_left(make_item("l2", StatusBarAlignment::Left, 20));
        layout.add_to_center(make_item("c1", StatusBarAlignment::Center, 5));
        layout.add_to_right(make_item("r1", StatusBarAlignment::Right, 1));

        let left = layout.get_left();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].id, "l2");
        assert_eq!(left[1].id, "l1");

        let center = layout.get_center();
        assert_eq!(center.len(), 1);
        assert_eq!(center[0].id, "c1");

        let right = layout.get_right();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].id, "r1");
    }

    #[test]
    fn layout_total_items() {
        let mut layout = StatusBarLayout::new();
        assert_eq!(layout.total_items(), 0);
        layout.add_to_left(make_item("l", StatusBarAlignment::Left, 1));
        layout.add_to_center(make_item("c", StatusBarAlignment::Center, 1));
        layout.add_to_right(make_item("r", StatusBarAlignment::Right, 1));
        assert_eq!(layout.total_items(), 3);
    }

    #[test]
    fn layout_remove_from_any_section() {
        let mut layout = StatusBarLayout::new();
        layout.add_to_left(make_item("a", StatusBarAlignment::Left, 1));
        layout.add_to_center(make_item("b", StatusBarAlignment::Center, 1));
        layout.add_to_right(make_item("c", StatusBarAlignment::Right, 1));

        layout.remove_item("b");
        assert_eq!(layout.total_items(), 2);
        assert!(layout.center.is_empty());

        layout.remove_item("c");
        assert_eq!(layout.total_items(), 1);
        assert!(layout.right.is_empty());
    }

    // -----------------------------------------------------------------------
    // animation_frame tests
    // -----------------------------------------------------------------------

    #[test]
    fn animation_frame_cycles_through_frames() {
        let frames = &["a", "b", "c"];
        assert_eq!(animation_frame(0, frames), "a");
        assert_eq!(animation_frame(80, frames), "b");
        assert_eq!(animation_frame(160, frames), "c");
        // wraps around
        assert_eq!(animation_frame(240, frames), "a");
    }

    #[test]
    fn animation_frame_empty_returns_default() {
        let frames: &[&str] = &[];
        assert_eq!(animation_frame(0, frames), "");
        assert_eq!(animation_frame(1000, frames), "");
    }

    #[test]
    fn animation_frame_with_spinner_frames() {
        assert_eq!(animation_frame(0, SPINNER_FRAMES), "⠋");
        assert_eq!(animation_frame(80, SPINNER_FRAMES), "⠙");
        // full cycle (10 frames × 80ms = 800ms)
        assert_eq!(animation_frame(800, SPINNER_FRAMES), "⠋");
    }

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_empty() {
        let mut svc = StatusBarService::new();
        assert!(svc.is_empty());
        svc.add_item(make_item("a", StatusBarAlignment::Left, 1));
        assert!(!svc.is_empty());
        svc.remove_item("a");
        assert!(svc.is_empty());
    }

    #[test]
    fn test_find_by_text() {
        let mut svc = StatusBarService::new();
        svc.add_item(StatusBarItem {
            id: "enc".into(),
            text: "UTF-8".into(),
            tooltip: None,
            command: None,
            alignment: StatusBarAlignment::Right,
            priority: 1,
            visible: true,
            background_color: None,
            foreground_color: None,
        });
        svc.add_item(StatusBarItem {
            id: "lang".into(),
            text: "Rust".into(),
            tooltip: None,
            command: None,
            alignment: StatusBarAlignment::Right,
            priority: 2,
            visible: true,
            background_color: None,
            foreground_color: None,
        });
        svc.add_item(StatusBarItem {
            id: "enc2".into(),
            text: "UTF-16".into(),
            tooltip: None,
            command: None,
            alignment: StatusBarAlignment::Left,
            priority: 3,
            visible: true,
            background_color: None,
            foreground_color: None,
        });

        let results = svc.find_by_text("UTF");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|i| i.id == "enc"));
        assert!(results.iter().any(|i| i.id == "enc2"));

        let none = svc.find_by_text("Python");
        assert!(none.is_empty());
    }

    #[test]
    fn test_clear_all() {
        let mut svc = StatusBarService::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _h = svc.on_did_change().on(move |_: &()| {
            *c.lock().unwrap() += 1;
        });

        svc.add_item(make_item("a", StatusBarAlignment::Left, 1));
        svc.add_item(make_item("b", StatusBarAlignment::Right, 2));
        assert_eq!(svc.item_count(), 2);

        svc.clear_all();
        assert!(svc.is_empty());
        assert_eq!(svc.item_count(), 0);
        // 2 adds + 1 clear = 3 events
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn test_clear_all_empty_no_event() {
        let mut svc = StatusBarService::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _h = svc.on_did_change().on(move |_: &()| {
            *c.lock().unwrap() += 1;
        });

        svc.clear_all();
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn test_has_command() {
        let with_cmd = StatusBarItemBuilder::new("cmd")
            .command("do.something")
            .build();
        assert!(with_cmd.has_command());

        let without_cmd = StatusBarItemBuilder::new("no_cmd").build();
        assert!(!without_cmd.has_command());
    }

    #[test]
    fn test_has_tooltip() {
        let with_tt = StatusBarItemBuilder::new("tt")
            .tooltip("A tip")
            .build();
        assert!(with_tt.has_tooltip());

        let without_tt = StatusBarItemBuilder::new("no_tt").build();
        assert!(!without_tt.has_tooltip());
    }

    #[test]
    fn test_display_for_item_shows_text_and_alignment() {
        let left_item = StatusBarItemBuilder::new("d1")
            .text("main")
            .alignment(StatusBarAlignment::Left)
            .build();
        assert_eq!(format!("{}", left_item), "main [Left]");

        let right_item = StatusBarItemBuilder::new("d2")
            .text("UTF-8")
            .alignment(StatusBarAlignment::Right)
            .build();
        assert_eq!(format!("{}", right_item), "UTF-8 [Right]");

        let center_item = StatusBarItemBuilder::new("d3")
            .text("Ready")
            .alignment(StatusBarAlignment::Center)
            .build();
        assert_eq!(format!("{}", center_item), "Ready [Center]");
    }

    // --- New tests ---

    #[test]
    fn notification_lifecycle() {
        let mut n = StatusBarNotification::new("n1", "Saved!", 3000)
            .with_priority(NotificationPriority::High)
            .with_icon("✓");
        assert!(!n.is_expired());
        assert_eq!(n.display_text(), "✓ Saved!");
        assert_eq!(n.remaining_ms(), 3000);
        n.tick(1500);
        assert_eq!(n.remaining_ms(), 1500);
        assert!(!n.is_expired());
        n.tick(1500);
        assert!(n.is_expired());
    }

    #[test]
    fn notification_queue_priority_and_sweep() {
        let mut q = NotificationQueue::new();
        q.push(StatusBarNotification::new("low", "low", 100).with_priority(NotificationPriority::Low));
        q.push(StatusBarNotification::new("urgent", "urgent", 100).with_priority(NotificationPriority::Urgent));
        assert_eq!(q.current().unwrap().id, "urgent");
        assert_eq!(q.len(), 2);
        let removed = q.tick(200);
        assert_eq!(removed, 2);
        assert!(q.is_empty());
    }

    #[test]
    fn notification_dismiss() {
        let mut q = NotificationQueue::new();
        q.push(StatusBarNotification::new("d", "dismiss me", 5000));
        assert!(q.dismiss("d"));
        assert!(!q.dismiss("nonexistent"));
        let removed = q.sweep();
        assert_eq!(removed, 1);
        assert!(q.is_empty());
    }

    #[test]
    fn progress_item_advance_and_display() {
        let mut p = StatusBarProgressItem::new("p1", "Building", 200);
        assert_eq!(p.percentage(), 0);
        assert!(!p.completed);
        p.advance(100);
        assert_eq!(p.percentage(), 50);
        assert_eq!(p.display_text(), "Building: 50%");
        p.advance(100);
        assert!(p.completed);
        assert_eq!(p.percentage(), 100);
    }

    #[test]
    fn progress_item_zero_total() {
        let p = StatusBarProgressItem::new("z", "Empty", 0);
        assert_eq!(p.ratio(), 1.0);
        assert_eq!(p.percentage(), 100);
    }

    #[test]
    fn context_menu_navigation_and_activate() {
        let mut menu = StatusBarContextMenu::new("item1");
        menu.add_action(ContextMenuAction::new("Copy", "editor.copy"));
        menu.add_action(ContextMenuAction::new("Paste", "editor.paste").disabled());
        menu.add_action(ContextMenuAction::new("Cut", "editor.cut"));
        menu.show();
        assert!(menu.visible);
        assert_eq!(menu.activate(), Some("editor.copy"));
        menu.select_next();
        assert_eq!(menu.activate(), None); // disabled
        menu.select_next();
        assert_eq!(menu.activate(), Some("editor.cut"));
        assert_eq!(menu.enabled_count(), 2);
    }

    #[test]
    fn context_menu_wrap_around() {
        let mut menu = StatusBarContextMenu::new("wrap");
        menu.add_action(ContextMenuAction::new("A", "a"));
        menu.add_action(ContextMenuAction::new("B", "b"));
        menu.select_prev(); // should wrap to last
        assert_eq!(menu.selected, 1);
        menu.select_next(); // wrap back to 0
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn layout_responsive_widths() {
        let mut layout = StatusBarLayout::new();
        layout.add_to_left(make_item("branch", StatusBarAlignment::Left, 10));
        layout.add_to_right(make_item("utf8", StatusBarAlignment::Right, 10));
        let (l, c, r) = layout.compute_responsive_widths(80, 1);
        assert!(l > 0);
        assert!(r > 0);
        assert_eq!(c, 0); // no center items
    }

    #[test]
    fn layout_find_item_across_sections() {
        let mut layout = StatusBarLayout::new();
        layout.add_to_left(make_item("left1", StatusBarAlignment::Left, 1));
        layout.add_to_right(make_item("right1", StatusBarAlignment::Right, 1));
        assert!(layout.find_item("left1").is_some());
        assert!(layout.find_item("right1").is_some());
        assert!(layout.find_item("nope").is_none());
    }

    #[test]
    fn layout_visible_count() {
        let mut layout = StatusBarLayout::new();
        layout.add_to_left(make_item("a", StatusBarAlignment::Left, 1));
        let mut hidden = make_item("b", StatusBarAlignment::Right, 1);
        hidden.visible = false;
        layout.add_to_right(hidden);
        assert_eq!(layout.visible_count(), 1);
    }

    #[test]
    fn item_summary_format() {
        let item = make_item("git", StatusBarAlignment::Left, 50);
        let s = item.summary();
        assert!(s.contains("git"));
        assert!(s.contains("Left"));
        assert!(s.contains("p=50"));
    }

    #[test]
    fn item_text_width() {
        let item = make_item("enc", StatusBarAlignment::Right, 10);
        assert_eq!(item.text_width(), 3); // "enc"
    }

    #[test]
    fn item_has_custom_colors() {
        let mut item = make_item("x", StatusBarAlignment::Left, 1);
        assert!(!item.has_custom_colors());
        item.background_color = Some("#ff0000".into());
        assert!(item.has_custom_colors());
    }

    #[test]
    fn service_items_sorted_by_priority() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("low", StatusBarAlignment::Left, 10));
        svc.add_item(make_item("high", StatusBarAlignment::Right, 100));
        svc.add_item(make_item("mid", StatusBarAlignment::Left, 50));
        let sorted = svc.items_sorted_by_priority();
        assert_eq!(sorted[0].id, "high");
        assert_eq!(sorted[1].id, "mid");
        assert_eq!(sorted[2].id, "low");
    }

    #[test]
    fn service_total_visible_text_width() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("ab", StatusBarAlignment::Left, 1));
        svc.add_item(make_item("cde", StatusBarAlignment::Right, 2));
        // text is same as id in make_item, so "ab" (2) + "cde" (3)
        assert_eq!(svc.total_visible_text_width(), 5);
    }

    #[test]
    fn service_active_alignments() {
        let mut svc = StatusBarService::new();
        svc.add_item(make_item("a", StatusBarAlignment::Left, 1));
        svc.add_item(make_item("b", StatusBarAlignment::Right, 1));
        let aligns = svc.active_alignments();
        assert!(aligns.contains(&StatusBarAlignment::Left));
        assert!(aligns.contains(&StatusBarAlignment::Right));
        assert!(!aligns.contains(&StatusBarAlignment::Center));
    }

    #[test]
    fn section_total_text_width() {
        let mut section = StatusBarSection::new("test", StatusBarAlignment::Left);
        section.add_item(make_item("abc", StatusBarAlignment::Left, 1));
        section.add_item(make_item("de", StatusBarAlignment::Left, 2));
        assert_eq!(section.total_text_width(), 5); // "abc" (3) + "de" (2)
    }

    #[test]
    fn section_visible_count() {
        let mut section = StatusBarSection::new("test", StatusBarAlignment::Left);
        section.add_item(make_item("a", StatusBarAlignment::Left, 1));
        let mut hidden = make_item("b", StatusBarAlignment::Left, 1);
        hidden.visible = false;
        section.add_item(hidden);
        assert_eq!(section.visible_count(), 1);
    }

    #[test]
    fn group_ids_display() {
        let mut g = StatusBarGroup::new("test");
        g.add("a");
        g.add("b");
        g.add("c");
        assert_eq!(g.ids_display(), "a, b, c");
    }

    #[test]
    fn notification_remaining_pct() {
        let n = StatusBarNotification::new("n1", "hello", 5000);
        assert!((n.remaining_pct() - 100.0).abs() < 0.1);
    }

    // -- StatusBarLanguageSelector -----------------------------------------

    #[test]
    fn language_selector_select() {
        let mut sel = StatusBarLanguageSelector::new("Rust");
        sel.set_available(vec!["Rust".into(), "Python".into(), "Go".into()]);
        assert!(sel.select("Python"));
        assert_eq!(sel.current_language, "Python");
        assert!(!sel.select("Unknown"));
    }

    #[test]
    fn language_selector_filter() {
        let mut sel = StatusBarLanguageSelector::new("Rust");
        sel.set_available(vec!["Rust".into(), "Ruby".into(), "Python".into()]);
        let filtered = sel.filter("Ru");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn language_selector_to_item() {
        let sel = StatusBarLanguageSelector::new("Rust");
        let item = sel.to_status_item("lang");
        assert_eq!(item.text, "Rust");
    }

    #[test]
    fn language_selector_display() {
        let sel = StatusBarLanguageSelector::new("Rust");
        assert_eq!(format!("{sel}"), "Language: Rust");
    }

    // -- StatusBarEncodingSelector -----------------------------------------

    #[test]
    fn encoding_selector_select() {
        let mut sel = StatusBarEncodingSelector::new("UTF-8");
        assert!(sel.select("ASCII"));
        assert_eq!(sel.current_encoding, "ASCII");
        assert!(!sel.select("Unknown-Encoding"));
    }

    #[test]
    fn encoding_selector_display() {
        let sel = StatusBarEncodingSelector::new("UTF-8");
        assert_eq!(format!("{sel}"), "Encoding: UTF-8");
    }

    // -- StatusBarLineEndingSelector ---------------------------------------

    #[test]
    fn line_ending_toggle() {
        let mut sel = StatusBarLineEndingSelector::new(LineEnding::LF);
        assert_eq!(sel.toggle(), LineEnding::CRLF);
        assert_eq!(sel.toggle(), LineEnding::LF);
    }

    #[test]
    fn line_ending_display() {
        let sel = StatusBarLineEndingSelector::new(LineEnding::CRLF);
        assert_eq!(format!("{sel}"), "EOL: CRLF");
    }

    #[test]
    fn line_ending_to_item() {
        let sel = StatusBarLineEndingSelector::new(LineEnding::LF);
        let item = sel.to_status_item("eol");
        assert_eq!(item.text, "LF");
    }

    // -- StatusBarActionDispatcher -----------------------------------------

    #[test]
    fn action_dispatcher_register_and_dispatch() {
        let mut disp = StatusBarActionDispatcher::new();
        disp.register(StatusBarClickAction::new("lang", "changeLanguage"));
        assert!(disp.has_action("lang"));
        let action = disp.dispatch("lang").unwrap();
        assert_eq!(action.command, "changeLanguage");
    }

    #[test]
    fn action_dispatcher_replace_existing() {
        let mut disp = StatusBarActionDispatcher::new();
        disp.register(StatusBarClickAction::new("lang", "old"));
        disp.register(StatusBarClickAction::new("lang", "new"));
        assert_eq!(disp.action_count(), 1);
        assert_eq!(disp.dispatch("lang").unwrap().command, "new");
    }

    #[test]
    fn action_dispatcher_display() {
        let disp = StatusBarActionDispatcher::default();
        assert!(format!("{disp}").contains("0 actions"));
    }

    #[test]
    fn click_action_display() {
        let a = StatusBarClickAction::new("id", "cmd");
        assert!(format!("{a}").contains("id"));
    }
}
