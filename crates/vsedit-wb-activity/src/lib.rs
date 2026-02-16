//! Activity bar.

use std::fmt;

/// Errors that can occur when operating on the activity bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityBarError {
    ItemNotFound(String),
    DuplicateItem(String),
    BarHidden,
}

impl fmt::Display for ActivityBarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ItemNotFound(id) => write!(f, "item not found: {id}"),
            Self::DuplicateItem(id) => write!(f, "duplicate item: {id}"),
            Self::BarHidden => write!(f, "activity bar is hidden"),
        }
    }
}

/// Position of the activity bar in the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityBarPosition {
    Side,
    Top,
    Hidden,
}

impl fmt::Display for ActivityBarPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Side => write!(f, "Side"),
            Self::Top => write!(f, "Top"),
            Self::Hidden => write!(f, "Hidden"),
        }
    }
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

impl fmt::Display for ActivityBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.id, self.title)
    }
}

/// Builder for constructing an [`ActivityBarItem`].
pub struct ActivityBarItemBuilder {
    id: String,
    title: String,
    icon: String,
    badge: Option<String>,
    active: bool,
    visible: bool,
    order: i32,
}

impl ActivityBarItemBuilder {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: String::new(),
            badge: None,
            active: false,
            visible: true,
            order: 0,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn build(self) -> ActivityBarItem {
        ActivityBarItem {
            id: self.id,
            title: self.title,
            icon: self.icon,
            badge: self.badge,
            active: self.active,
            visible: self.visible,
            order: self.order,
        }
    }
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

    /// Adds an item, returning an error if an item with the same id already exists.
    pub fn try_add_item(&mut self, item: ActivityBarItem) -> Result<(), ActivityBarError> {
        if self.items.iter().any(|i| i.id == item.id) {
            return Err(ActivityBarError::DuplicateItem(item.id));
        }
        self.items.push(item);
        Ok(())
    }

    pub fn get_item(&self, id: &str) -> Option<&ActivityBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn set_visibility(&mut self, id: &str, visible: bool) -> Result<(), ActivityBarError> {
        self.items
            .iter_mut()
            .find(|i| i.id == id)
            .map(|item| item.visible = visible)
            .ok_or_else(|| ActivityBarError::ItemNotFound(id.to_string()))
    }

    /// Sorts items by their `order` field (ascending).
    pub fn sort_items(&mut self) {
        self.items.sort_by_key(|i| i.order);
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn clear_all_badges(&mut self) {
        for item in &mut self.items {
            item.badge = None;
        }
    }

    /// Returns items whose title contains the given substring (case-insensitive).
    pub fn find_by_title(&self, query: &str) -> Vec<&ActivityBarItem> {
        let query_lower = query.to_lowercase();
        self.items
            .iter()
            .filter(|i| i.title.to_lowercase().contains(&query_lower))
            .collect()
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

    #[test]
    fn try_add_item_rejects_duplicate() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        let result = bar.try_add_item(make_item("explorer", 1));
        assert_eq!(result, Err(ActivityBarError::DuplicateItem("explorer".to_string())));
    }

    #[test]
    fn try_add_item_succeeds_for_unique() {
        let mut bar = ActivityBar::new();
        assert!(bar.try_add_item(make_item("explorer", 0)).is_ok());
        assert!(bar.try_add_item(make_item("search", 1)).is_ok());
        assert_eq!(bar.item_count(), 2);
    }

    #[test]
    fn get_item_found_and_missing() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.get_item("explorer").is_some());
        assert!(bar.get_item("missing").is_none());
    }

    #[test]
    fn set_visibility_updates_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.set_visibility("explorer", false).is_ok());
        assert!(!bar.get_item("explorer").unwrap().visible);
        assert_eq!(bar.get_visible_items().len(), 0);
    }

    #[test]
    fn set_visibility_returns_error_for_missing() {
        let mut bar = ActivityBar::new();
        let result = bar.set_visibility("nope", true);
        assert_eq!(result, Err(ActivityBarError::ItemNotFound("nope".to_string())));
    }

    #[test]
    fn sort_items_by_order() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("c", 3));
        bar.add_item(make_item("a", 1));
        bar.add_item(make_item("b", 2));
        bar.sort_items();
        let ids: Vec<&str> = bar.get_visible_items().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn item_count_reflects_additions_and_removals() {
        let mut bar = ActivityBar::new();
        assert_eq!(bar.item_count(), 0);
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        assert_eq!(bar.item_count(), 2);
        bar.remove_item("a");
        assert_eq!(bar.item_count(), 1);
    }

    #[test]
    fn clear_all_badges() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.set_badge("a", Some("5".to_string()));
        bar.set_badge("b", Some("!".to_string()));
        bar.clear_all_badges();
        assert!(bar.get_item("a").unwrap().badge.is_none());
        assert!(bar.get_item("b").unwrap().badge.is_none());
    }

    #[test]
    fn find_by_title_case_insensitive() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        bar.add_item(make_item("search", 1));
        let results = bar.find_by_title("ITEM");
        assert_eq!(results.len(), 2);
        let results = bar.find_by_title("explorer");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "explorer");
    }

    #[test]
    fn find_by_title_returns_empty_on_no_match() {
        let bar = ActivityBar::new();
        assert!(bar.find_by_title("nothing").is_empty());
    }

    #[test]
    fn builder_creates_item_with_defaults() {
        let item = ActivityBarItemBuilder::new("ext", "Extensions")
            .icon("puzzle")
            .order(3)
            .build();
        assert_eq!(item.id, "ext");
        assert_eq!(item.title, "Extensions");
        assert_eq!(item.icon, "puzzle");
        assert_eq!(item.order, 3);
        assert!(item.visible);
        assert!(!item.active);
        assert!(item.badge.is_none());
    }

    #[test]
    fn builder_with_all_fields() {
        let item = ActivityBarItemBuilder::new("scm", "Source Control")
            .icon("git")
            .badge("2")
            .active(true)
            .visible(false)
            .order(5)
            .build();
        assert_eq!(item.badge.as_deref(), Some("2"));
        assert!(item.active);
        assert!(!item.visible);
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", ActivityBarPosition::Side), "Side");
        assert_eq!(format!("{}", ActivityBarPosition::Top), "Top");
        assert_eq!(format!("{}", ActivityBarPosition::Hidden), "Hidden");

        let item = make_item("explorer", 0);
        assert_eq!(format!("{}", item), "[explorer] Item explorer");
    }

    #[test]
    fn error_display() {
        let e = ActivityBarError::ItemNotFound("x".into());
        assert_eq!(format!("{e}"), "item not found: x");
        let e = ActivityBarError::DuplicateItem("y".into());
        assert_eq!(format!("{e}"), "duplicate item: y");
        let e = ActivityBarError::BarHidden;
        assert_eq!(format!("{e}"), "activity bar is hidden");
    }
}
