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
}
