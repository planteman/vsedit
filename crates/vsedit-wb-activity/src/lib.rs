//! Activity bar.

/// Position of the activity bar in the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityBarPosition {
    Side,
    Top,
    Hidden,
}

/// An item displayed in the activity bar.
#[derive(Debug, Clone)]
pub struct ActivityBarItem {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub badge: Option<String>,
    pub active: bool,
    pub visible: bool,
    pub order: i32,
}

/// The activity bar containing sidebar navigation items.
pub struct ActivityBar {
    items: Vec<ActivityBarItem>,
    position: ActivityBarPosition,
}

impl ActivityBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            position: ActivityBarPosition::Side,
        }
    }

    pub fn add_item(&mut self, item: ActivityBarItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        let len = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != len
    }

    /// Activates the item with the given id, deactivating all others.
    pub fn activate(&mut self, id: &str) {
        for item in &mut self.items {
            item.active = item.id == id;
        }
    }

    pub fn get_active(&self) -> Option<&ActivityBarItem> {
        self.items.iter().find(|i| i.active)
    }

    pub fn set_badge(&mut self, id: &str, badge: Option<String>) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.badge = badge;
        }
    }

    pub fn set_position(&mut self, position: ActivityBarPosition) {
        self.position = position;
    }

    pub fn position(&self) -> ActivityBarPosition {
        self.position
    }

    pub fn get_visible_items(&self) -> Vec<&ActivityBarItem> {
        self.items.iter().filter(|i| i.visible).collect()
    }
}

impl Default for ActivityBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: &str, order: i32) -> ActivityBarItem {
        ActivityBarItem {
            id: id.to_string(),
            title: format!("Item {id}"),
            icon: "icon".to_string(),
            badge: None,
            active: false,
            visible: true,
            order,
        }
    }

    #[test]
    fn add_and_activate() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        bar.add_item(make_item("search", 1));
        bar.activate("search");
        let active = bar.get_active().unwrap();
        assert_eq!(active.id, "search");
    }

    #[test]
    fn remove_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.remove_item("explorer"));
        assert!(!bar.remove_item("explorer"));
    }

    #[test]
    fn visible_items_and_badge() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        let mut hidden = make_item("debug", 1);
        hidden.visible = false;
        bar.add_item(hidden);
        assert_eq!(bar.get_visible_items().len(), 1);
        bar.set_badge("explorer", Some("3".to_string()));
        assert_eq!(
            bar.get_visible_items()[0].badge.as_deref(),
            Some("3")
        );
    }

    #[test]
    fn set_position() {
        let mut bar = ActivityBar::new();
        assert_eq!(bar.position(), ActivityBarPosition::Side);
        bar.set_position(ActivityBarPosition::Hidden);
        assert_eq!(bar.position(), ActivityBarPosition::Hidden);
    }
}
