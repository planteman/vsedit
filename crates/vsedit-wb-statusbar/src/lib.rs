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
            StatusBarAlignment::Right => write!(f, "Right"),
        }
    }
}

impl fmt::Display for StatusBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({:?}, pri={})",
            self.id, self.text, self.alignment, self.priority
        )
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
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "statusbar.branch");

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
        assert_eq!(display, "[sb.test] branch: main (Left, pri=10)");
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
}
