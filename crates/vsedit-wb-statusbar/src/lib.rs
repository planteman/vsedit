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

use std::collections::HashMap;
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


// ---------------------------------------------------------------------------
// StatusbarTooltipBuilder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatusbarTooltipBuilder {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl StatusbarTooltipBuilder {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for StatusbarTooltipBuilder {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for StatusbarTooltipBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StatusbarTooltipBuilder({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// StatusbarCommandRunner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatusbarCommandRunner {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl StatusbarCommandRunner {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for StatusbarCommandRunner {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for StatusbarCommandRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StatusbarCommandRunner({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// StatusbarTooltipBuilderSnapshot — point-in-time snapshot of StatusbarTooltipBuilder state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatusbarTooltipBuilderSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl StatusbarTooltipBuilderSnapshot {
    pub fn capture(source: &StatusbarTooltipBuilder, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for StatusbarTooltipBuilderSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// StatusbarCommandRunnerStats — aggregate statistics for StatusbarCommandRunner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StatusbarCommandRunnerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl StatusbarCommandRunnerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for StatusbarCommandRunnerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// StatusbarTooltipBuilderConfig — configuration for StatusbarTooltipBuilder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatusbarTooltipBuilderConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl StatusbarTooltipBuilderConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for StatusbarTooltipBuilderConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for StatusbarTooltipBuilderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// StatusBarLayoutEngine
// ---------------------------------------------------------------------------

/// Compute status bar item positions.
#[derive(Debug, Clone)]
pub struct StatusBarLayoutItem {
    pub id: String,
    pub alignment: StatusBarAlignment,
    pub width: u32,
    pub x: u32,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct StatusBarLayoutEngine {
    available_width: u32,
}

impl StatusBarLayoutEngine {
    pub fn new(available_width: u32) -> Self {
        Self { available_width }
    }

    pub fn available_width(&self) -> u32 {
        self.available_width
    }

    pub fn reflow_items(
        &self,
        items: &[(String, StatusBarAlignment, u32)],
    ) -> Vec<StatusBarLayoutItem> {
        let mut result = Vec::new();
        let mut left_x: u32 = 0;
        let mut right_x: u32 = self.available_width;

        for (id, align, width) in items {
            match align {
                StatusBarAlignment::Left => {
                    let truncated = left_x + width > self.available_width;
                    result.push(StatusBarLayoutItem {
                        id: id.clone(),
                        alignment: *align,
                        width: *width,
                        x: left_x,
                        truncated,
                    });
                    left_x += width;
                }
                StatusBarAlignment::Right => {
                    right_x = right_x.saturating_sub(*width);
                    result.push(StatusBarLayoutItem {
                        id: id.clone(),
                        alignment: *align,
                        width: *width,
                        x: right_x,
                        truncated: right_x < left_x,
                    });
                }
                StatusBarAlignment::Center => {
                    let cx = self.available_width / 2 - width / 2;
                    result.push(StatusBarLayoutItem {
                        id: id.clone(),
                        alignment: *align,
                        width: *width,
                        x: cx,
                        truncated: false,
                    });
                }
            }
        }
        result
    }

    pub fn item_at_x<'a>(&self, items: &'a [StatusBarLayoutItem], x: u32) -> Option<&'a str> {
        items
            .iter()
            .find(|item| x >= item.x && x < item.x + item.width)
            .map(|item| item.id.as_str())
    }

    pub fn overflow_detected(&self, items: &[StatusBarLayoutItem]) -> bool {
        items.iter().any(|i| i.truncated)
    }

    pub fn truncation_order(items: &mut [StatusBarLayoutItem]) {
        items.sort_by(|a, b| b.width.cmp(&a.width));
    }
}

// ---------------------------------------------------------------------------
// StatusBarAnimation
// ---------------------------------------------------------------------------

/// Animate status bar transitions.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationState {
    FadeIn,
    FadeOut,
    Pulse,
    Idle,
}

#[derive(Debug, Clone)]
pub struct StatusBarAnimation {
    state: AnimationState,
    progress: f64,
}

impl StatusBarAnimation {
    pub fn new() -> Self {
        Self {
            state: AnimationState::Idle,
            progress: 0.0,
        }
    }

    pub fn fade_in(&mut self) {
        self.state = AnimationState::FadeIn;
        self.progress = 0.0;
    }

    pub fn fade_out(&mut self) {
        self.state = AnimationState::FadeOut;
        self.progress = 1.0;
    }

    pub fn pulse(&mut self) {
        self.state = AnimationState::Pulse;
        self.progress = 0.0;
    }

    pub fn progress(&self) -> f64 {
        self.progress
    }

    pub fn is_animating(&self) -> bool {
        self.state != AnimationState::Idle
    }

    pub fn tick_animation(&mut self, delta: f64) {
        match self.state {
            AnimationState::FadeIn => {
                self.progress = (self.progress + delta).min(1.0);
                if self.progress >= 1.0 {
                    self.state = AnimationState::Idle;
                }
            }
            AnimationState::FadeOut => {
                self.progress = (self.progress - delta).max(0.0);
                if self.progress <= 0.0 {
                    self.state = AnimationState::Idle;
                }
            }
            AnimationState::Pulse => {
                self.progress = (self.progress + delta) % 1.0;
            }
            AnimationState::Idle => {}
        }
    }

    pub fn animation_complete(&self) -> bool {
        self.state == AnimationState::Idle
    }
}

// ---------------------------------------------------------------------------
// StatusBarTooltip
// ---------------------------------------------------------------------------

/// Rich tooltip for a status bar item.
#[derive(Debug, Clone)]
pub struct StatusBarTooltip {
    pub title: String,
    pub body: Option<String>,
    pub command: Option<String>,
}

impl StatusBarTooltip {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            body: None,
            command: None,
        }
    }

    pub fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub fn with_command(mut self, cmd: &str) -> Self {
        self.command = Some(cmd.to_string());
        self
    }

    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }

    pub fn format_tooltip(&self) -> String {
        let mut result = self.title.clone();
        if let Some(ref body) = self.body {
            result.push_str("\n");
            result.push_str(body);
        }
        result
    }

    pub fn truncated_text(&self, max_length: usize) -> String {
        if self.title.len() <= max_length {
            self.title.clone()
        } else {
            format!("{}...", &self.title[..max_length.saturating_sub(3)])
        }
    }
}


/// Configuration manager for wb_statusbar functionality.
pub struct WbStatusbarConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbStatusbarConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &WbStatusbarConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_statusbar operations.
pub struct WbStatusbarRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbStatusbarRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for wb_statusbar.
pub struct WbStatusbarValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbStatusbarValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &WbStatusbarValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_statusbar
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbStatusbarRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbStatusbarRingBuf {
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
pub struct XaWbStatusbarCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbStatusbarCounter {
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

impl Default for XaWbStatusbarCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 225
// ---------------------------------------------------------------------------

/// Generic object pool `Xc225Pool<T>`.
pub struct Xc225Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc225Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc225PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc225Pool<T> {
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
    pub fn stats(&self) -> Xc225PoolStats {
        Xc225PoolStats {
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

impl<T> Default for Xc225Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc225Scheduler`.
pub struct Xc225Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc225Scheduler {
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

impl Default for Xc225Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_225 hash for the given byte slice.
pub fn xc_225_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_225 convention.
pub fn xc_225_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_88 deepening: state machine + event bus ---

/// States for the Xd88 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd88State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd88State {
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
pub struct Xd88Transition {
    pub from: Xd88State,
    pub to: Xd88State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd88StateMachine {
    current: Xd88State,
    history: Vec<Xd88Transition>,
    step_counter: usize,
}

impl Xd88StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd88State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd88State {
        self.current
    }

    pub fn history(&self) -> &[Xd88Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd88State) -> Result<Xd88State, String> {
        let allowed = match (self.current, target) {
            (Xd88State::Idle, Xd88State::Running) => true,
            (Xd88State::Running, Xd88State::Paused) => true,
            (Xd88State::Running, Xd88State::Done) => true,
            (Xd88State::Paused, Xd88State::Running) => true,
            (Xd88State::Paused, Xd88State::Done) => true,
            (Xd88State::Done, Xd88State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_88: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd88Transition {
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
            "Xd88SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd88State> {
        let prefix = "Xd88SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd88State::Idle),
            "Running" => Some(Xd88State::Running),
            "Paused" => Some(Xd88State::Paused),
            "Done" => Some(Xd88State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd88State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd88 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd88Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd88Event {
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

type Xd88HandlerFn = Box<dyn Fn(&Xd88Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd88EventBus {
    handlers: Vec<(usize, Option<String>, Xd88HandlerFn)>,
    next_id: usize,
    published: Vec<Xd88Event>,
}

impl Xd88EventBus {
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
        F: Fn(&Xd88Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd88Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd88Event) {
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

    pub fn published_events(&self) -> &[Xd88Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #113
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf113Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf113TrieNode {
    children: std::collections::HashMap<char, Xf113TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf113Trie {
    root: Xf113TrieNode,
    count: usize,
}

impl Xf113Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf113TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf113TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf113TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf113BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf113BloomFilter {
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

    #[test] fn statusbarTooltipBuilder_new() { let s = StatusbarTooltipBuilder::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn statusbarTooltipBuilder_add() { let mut s = StatusbarTooltipBuilder::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn statusbarTooltipBuilder_remove() { let mut s = StatusbarTooltipBuilder::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn statusbarTooltipBuilder_config() { let mut s = StatusbarTooltipBuilder::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn statusbarTooltipBuilder_nav() { let mut s = StatusbarTooltipBuilder::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn statusbarTooltipBuilder_filter() { let mut s = StatusbarTooltipBuilder::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn statusbarTooltipBuilder_display() { assert!(format!("{}", StatusbarTooltipBuilder::new()).contains("StatusbarTooltipBuilder")); }
    #[test] fn statusbarCommandRunner_new() { let s = StatusbarCommandRunner::new(); assert!(s.is_empty()); }
    #[test] fn statusbarCommandRunner_add() { let mut s = StatusbarCommandRunner::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn statusbarCommandRunner_active() { let mut s = StatusbarCommandRunner::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn statusbarCommandRunner_error() { let mut s = StatusbarCommandRunner::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn statusbarCommandRunner_rm_group() { let mut s = StatusbarCommandRunner::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn statusbarCommandRunner_display() { assert!(format!("{}", StatusbarCommandRunner::new()).contains("StatusbarCommandRunner")); }


    #[test] fn statusbarTooltipBuilder_snap_capture() {
        let s = StatusbarTooltipBuilder::new();
        let snap = StatusbarTooltipBuilderSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn statusbarTooltipBuilder_snap_stale() {
        let s = StatusbarTooltipBuilder::new();
        let snap = StatusbarTooltipBuilderSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn statusbarTooltipBuilder_snap_diff() {
        let s = StatusbarTooltipBuilder::new();
        let s1v = StatusbarTooltipBuilderSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn statusbarTooltipBuilder_snap_display() {
        let s = StatusbarTooltipBuilder::new();
        let snap = StatusbarTooltipBuilderSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn statusbarCommandRunner_stats_record() {
        let mut st = StatusbarCommandRunnerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn statusbarCommandRunner_stats_hit_ratio() {
        let mut st = StatusbarCommandRunnerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn statusbarCommandRunner_stats_merge() {
        let mut a = StatusbarCommandRunnerStats::new();
        a.total_adds = 5;
        let mut b = StatusbarCommandRunnerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn statusbarCommandRunner_stats_display() {
        let st = StatusbarCommandRunnerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn statusbarTooltipBuilder_config_default() {
        let c = StatusbarTooltipBuilderConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn statusbarTooltipBuilder_config_builder() {
        let c = StatusbarTooltipBuilderConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn statusbarTooltipBuilder_config_labels() {
        let mut c = StatusbarTooltipBuilderConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn statusbarTooltipBuilder_config_cleanup_threshold() {
        let c = StatusbarTooltipBuilderConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn statusbarTooltipBuilder_config_display() {
        assert!(format!("{}", StatusbarTooltipBuilderConfig::new()).contains("Config"));
    }
    #[test] fn statusbarCommandRunner_stats_peaks() {
        let mut st = StatusbarCommandRunnerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- StatusBarLayoutEngine ---------------------------------------------

    #[test]
    fn layout_engine_reflow() {
        let engine = StatusBarLayoutEngine::new(1000);
        let items = vec![
            ("branch".into(), StatusBarAlignment::Left, 100),
            ("errors".into(), StatusBarAlignment::Right, 80),
        ];
        let result = engine.reflow_items(&items);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].x, 0);
    }

    #[test]
    fn layout_engine_item_at_x() {
        let engine = StatusBarLayoutEngine::new(500);
        let items = vec![StatusBarLayoutItem {
            id: "test".into(),
            alignment: StatusBarAlignment::Left,
            width: 100,
            x: 0,
            truncated: false,
        }];
        assert_eq!(engine.item_at_x(&items, 50), Some("test"));
        assert_eq!(engine.item_at_x(&items, 150), None);
    }

    #[test]
    fn layout_engine_overflow() {
        let engine = StatusBarLayoutEngine::new(50);
        let items = vec![StatusBarLayoutItem {
            id: "big".into(),
            alignment: StatusBarAlignment::Left,
            width: 100,
            x: 0,
            truncated: true,
        }];
        assert!(engine.overflow_detected(&items));
    }

    #[test]
    fn layout_engine_available_width() {
        let engine = StatusBarLayoutEngine::new(800);
        assert_eq!(engine.available_width(), 800);
    }

    // -- StatusBarAnimation ------------------------------------------------

    #[test]
    fn animation_fade_in() {
        let mut anim = StatusBarAnimation::new();
        anim.fade_in();
        assert!(anim.is_animating());
        anim.tick_animation(0.5);
        assert!((anim.progress() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn animation_fade_in_complete() {
        let mut anim = StatusBarAnimation::new();
        anim.fade_in();
        anim.tick_animation(1.5);
        assert!(anim.animation_complete());
    }

    #[test]
    fn animation_fade_out() {
        let mut anim = StatusBarAnimation::new();
        anim.fade_out();
        anim.tick_animation(0.5);
        assert!((anim.progress() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn animation_idle_by_default() {
        let anim = StatusBarAnimation::new();
        assert!(!anim.is_animating());
        assert!(anim.animation_complete());
    }

    // -- StatusBarTooltip --------------------------------------------------

    #[test]
    fn tooltip_basic() {
        let tt = StatusBarTooltip::new("Git Branch").with_body("main");
        assert_eq!(tt.format_tooltip(), "Git Branch\nmain");
    }

    #[test]
    fn tooltip_has_command() {
        let tt = StatusBarTooltip::new("Errors").with_command("workbench.showErrors");
        assert!(tt.has_command());
    }

    #[test]
    fn tooltip_truncated() {
        let tt = StatusBarTooltip::new("This is a very long tooltip title text");
        let truncated = tt.truncated_text(15);
        assert!(truncated.len() <= 15);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn tooltip_no_command() {
        let tt = StatusBarTooltip::new("Info");
        assert!(!tt.has_command());
    }


    #[test]
    fn wb_statusbar_config_new() {
        let cfg = WbStatusbarConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_statusbar_config_set_get() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_statusbar_config_remove() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_statusbar_config_keys_sorted() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_statusbar_config_bump_version() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_statusbar_config_clear() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_statusbar_config_merge() {
        let mut cfg1 = WbStatusbarConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbStatusbarConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_statusbar_config_disable() {
        let mut cfg = WbStatusbarConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_statusbar_rate_tracker_empty() {
        let rt = WbStatusbarRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_statusbar_rate_tracker_record() {
        let mut rt = WbStatusbarRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_statusbar_rate_tracker_prune() {
        let mut rt = WbStatusbarRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_statusbar_validator_valid() {
        let v = WbStatusbarValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_statusbar_validator_errors() {
        let mut v = WbStatusbarValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_statusbar_validator_clear() {
        let mut v = WbStatusbarValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_statusbar_validator_merge() {
        let mut v1 = WbStatusbarValidator::new();
        v1.add_error("e1");
        let mut v2 = WbStatusbarValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_statusbar_rate_tracker_clear() {
        let mut rt = WbStatusbarRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for wb_statusbar
    #[test]
    fn xa_wb_statusbar_ring_new() {
        let rb = super::XaWbStatusbarRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_statusbar_ring_push_len() {
        let mut rb = super::XaWbStatusbarRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_statusbar_ring_wrap() {
        let mut rb = super::XaWbStatusbarRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_statusbar_ring_mean_empty() {
        let rb = super::XaWbStatusbarRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_statusbar_ring_mean_values() {
        let mut rb = super::XaWbStatusbarRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_statusbar_ring_min_max() {
        let mut rb = super::XaWbStatusbarRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_statusbar_ring_iter() {
        let mut rb = super::XaWbStatusbarRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_statusbar_counter_new() {
        let c = super::XaWbStatusbarCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_statusbar_counter_inc() {
        let mut c = super::XaWbStatusbarCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_statusbar_counter_inc_by() {
        let mut c = super::XaWbStatusbarCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_statusbar_counter_reset() {
        let mut c = super::XaWbStatusbarCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_statusbar_counter_clear() {
        let mut c = super::XaWbStatusbarCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_statusbar_counter_default() {
        let c = super::XaWbStatusbarCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 225 ----

    #[test]
    fn xc_225_pool_new_empty() {
        let pool: super::Xc225Pool<i32> = super::Xc225Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_225_pool_release_acquire() {
        let mut pool = super::Xc225Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_225_pool_acquire_empty() {
        let mut pool: super::Xc225Pool<i32> = super::Xc225Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_225_pool_full() {
        let mut pool = super::Xc225Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_225_pool_drain() {
        let mut pool = super::Xc225Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_225_pool_stats() {
        let mut pool = super::Xc225Pool::new(8);
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
    fn xc_225_pool_clear() {
        let mut pool = super::Xc225Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_225_pool_shrink() {
        let mut pool = super::Xc225Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_225_pool_default() {
        let pool: super::Xc225Pool<String> = super::Xc225Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_225_pool_extend() {
        let mut pool = super::Xc225Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_225_pool_retain() {
        let mut pool = super::Xc225Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_225_scheduler_round_robin() {
        let mut sched = super::Xc225Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_225_scheduler_empty() {
        let mut sched = super::Xc225Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_225_scheduler_reset() {
        let mut sched = super::Xc225Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_225_scheduler_add_remove() {
        let mut sched = super::Xc225Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_225_scheduler_targets() {
        let sched = super::Xc225Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_225_hash_empty() {
        assert_eq!(super::xc_225_hash(b""), 5381);
    }

    #[test]
    fn xc_225_hash_data() {
        let h = super::xc_225_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_225_hash(b"hello"), h);
    }

    #[test]
    fn xc_225_reverse_str() {
        assert_eq!(super::xc_225_reverse("abc"), "cba");
        assert_eq!(super::xc_225_reverse(""), "");
    }


    // --- xd_88 deepening tests ---

    #[test]
    fn xd_88_sm_initial_state() {
        let sm = Xd88StateMachine::new();
        assert_eq!(sm.current_state(), Xd88State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_88_sm_valid_idle_to_running() {
        let mut sm = Xd88StateMachine::new();
        assert!(sm.transition(Xd88State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd88State::Running);
    }

    #[test]
    fn xd_88_sm_valid_running_to_paused() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        assert!(sm.transition(Xd88State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd88State::Paused);
    }

    #[test]
    fn xd_88_sm_valid_running_to_done() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        assert!(sm.transition(Xd88State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd88State::Done);
    }

    #[test]
    fn xd_88_sm_valid_paused_to_running() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        sm.transition(Xd88State::Paused).unwrap();
        assert!(sm.transition(Xd88State::Running).is_ok());
    }

    #[test]
    fn xd_88_sm_valid_done_to_idle() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        sm.transition(Xd88State::Done).unwrap();
        assert!(sm.transition(Xd88State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd88State::Idle);
    }

    #[test]
    fn xd_88_sm_invalid_idle_to_done() {
        let mut sm = Xd88StateMachine::new();
        assert!(sm.transition(Xd88State::Done).is_err());
    }

    #[test]
    fn xd_88_sm_invalid_idle_to_paused() {
        let mut sm = Xd88StateMachine::new();
        assert!(sm.transition(Xd88State::Paused).is_err());
    }

    #[test]
    fn xd_88_sm_history_tracking() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        sm.transition(Xd88State::Paused).unwrap();
        sm.transition(Xd88State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd88State::Idle);
        assert_eq!(sm.history()[0].to, Xd88State::Running);
        assert_eq!(sm.history()[1].from, Xd88State::Running);
        assert_eq!(sm.history()[2].to, Xd88State::Done);
    }

    #[test]
    fn xd_88_sm_serialize_deserialize() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd88StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd88State::Running));
    }

    #[test]
    fn xd_88_sm_deserialize_invalid() {
        assert_eq!(Xd88StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_88_sm_reset() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd88State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_88_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd88EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd88Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_88_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd88EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd88Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd88Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_88_bus_unsubscribe() {
        let mut bus = Xd88EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_88_event_kind_and_payload() {
        let e = Xd88Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd88Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_88_bus_clear_history() {
        let mut bus = Xd88EventBus::new();
        bus.publish(Xd88Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_88_sm_step_counter_increments() {
        let mut sm = Xd88StateMachine::new();
        sm.transition(Xd88State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd88State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #113 --

    #[test]
    fn xf113_trie_insert_search() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf113_trie_starts_with() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf113_trie_remove() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf113_trie_word_count() {
        let mut t = Xf113Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf113_trie_longest_prefix() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf113_trie_all_words() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf113_trie_autocomplete() {
        let mut t = Xf113Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf113_trie_empty_search() {
        let t = Xf113Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf113_bloom_add_contains() {
        let mut bf = Xf113BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf113_bloom_probably_absent() {
        let bf = Xf113BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf113_bloom_false_positive_rate() {
        let mut bf = Xf113BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf113_bloom_clear() {
        let mut bf = Xf113BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf113_bloom_union() {
        let mut a = Xf113BloomFilter::xf_new(512, 2);
        let mut b = Xf113BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf113_bloom_intersection_estimate() {
        let mut a = Xf113BloomFilter::xf_new(512, 2);
        let mut b = Xf113BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf113_bloom_union_size_mismatch() {
        let a = Xf113BloomFilter::xf_new(256, 2);
        let b = Xf113BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
