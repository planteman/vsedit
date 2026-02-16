//! Editor tab bar widget.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabKind {
    File,
    Preview,
    Diff,
    Settings,
    Welcome,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub uri: Option<String>,
    pub kind: TabKind,
    pub dirty: bool,
    pub pinned: bool,
    pub preview: bool,
    pub active: bool,
}

pub struct TabGroup {
    tabs: Vec<Tab>,
    active_tab: Option<usize>,
}

impl TabGroup {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
    }

    pub fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }

    pub fn close_tab(&mut self, id: &str) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(pos);
            match self.active_tab {
                Some(idx) if idx == pos => {
                    self.active_tab = if self.tabs.is_empty() {
                        None
                    } else {
                        Some(idx.min(self.tabs.len() - 1))
                    };
                }
                Some(idx) if idx > pos => self.active_tab = Some(idx - 1),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    pub fn activate_tab(&mut self, id: &str) {
        for tab in &mut self.tabs {
            tab.active = false;
        }
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs[pos].active = true;
            self.active_tab = Some(pos);
        }
    }

    pub fn get_active_tab(&self) -> Option<&Tab> {
        self.active_tab.and_then(|i| self.tabs.get(i))
    }

    pub fn pin_tab(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = true;
        }
    }

    pub fn unpin_tab(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = false;
        }
    }

    pub fn mark_dirty(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = true;
        }
    }

    pub fn mark_clean(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = false;
        }
    }

    pub fn close_saved_tabs(&mut self) {
        let active_id = self.get_active_tab().map(|t| t.id.clone());
        self.tabs.retain(|t| t.dirty || t.pinned);
        self.active_tab = active_id.and_then(|id| self.tabs.iter().position(|t| t.id == id));
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_dirty_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.dirty).collect()
    }

    pub fn move_tab(&mut self, id: &str, new_index: usize) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            if new_index >= self.tabs.len() {
                return false;
            }
            let tab = self.tabs.remove(pos);
            self.tabs.insert(new_index, tab);
            // Update active_tab index to follow the active tab.
            self.active_tab = self.tabs.iter().position(|t| t.active);
            true
        } else {
            false
        }
    }

    pub fn get_tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn close_all(&mut self) -> Vec<Tab> {
        self.active_tab = None;
        std::mem::take(&mut self.tabs)
    }

    pub fn close_others(&mut self, id: &str) -> Vec<Tab> {
        let mut closed = Vec::new();
        let mut kept = Vec::new();
        for tab in self.tabs.drain(..) {
            if tab.id == id {
                kept.push(tab);
            } else {
                closed.push(tab);
            }
        }
        self.tabs = kept;
        self.active_tab = if self.tabs.is_empty() { None } else { Some(0) };
        closed
    }

    pub fn close_to_right(&mut self, id: &str) -> Vec<Tab> {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let closed: Vec<Tab> = self.tabs.drain(pos + 1..).collect();
            self.active_tab = self.tabs.iter().position(|t| t.active);
            closed
        } else {
            Vec::new()
        }
    }

    pub fn close_to_left(&mut self, id: &str) -> Vec<Tab> {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let closed: Vec<Tab> = self.tabs.drain(..pos).collect();
            self.active_tab = self.tabs.iter().position(|t| t.active);
            closed
        } else {
            Vec::new()
        }
    }

    pub fn get_pinned_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.pinned).collect()
    }

    pub fn get_preview_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.preview).collect()
    }

    pub fn promote_preview(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.preview = false;
        }
    }

    pub fn find_by_uri(&self, uri: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.uri.as_deref() == Some(uri))
    }
}

impl Default for TabGroup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSizing {
    Fit,
    Fixed,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseButtonPosition {
    Left,
    Right,
    Off,
}

#[derive(Debug, Clone)]
pub struct TabBarConfig {
    pub show_icons: bool,
    pub tab_sizing: TabSizing,
    pub close_button_position: CloseButtonPosition,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            show_icons: true,
            tab_sizing: TabSizing::Fit,
            close_button_position: CloseButtonPosition::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tab(id: &str) -> Tab {
        Tab {
            id: id.to_string(),
            label: id.to_string(),
            uri: None,
            kind: TabKind::File,
            dirty: false,
            pinned: false,
            preview: false,
            active: false,
        }
    }

    fn make_tab_with_uri(id: &str, uri: &str) -> Tab {
        Tab {
            id: id.to_string(),
            label: id.to_string(),
            uri: Some(uri.to_string()),
            kind: TabKind::File,
            dirty: false,
            pinned: false,
            preview: false,
            active: false,
        }
    }

    #[test]
    fn add_activate_close() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.activate_tab("b");
        assert_eq!(group.get_active_tab().unwrap().id, "b");
        assert!(group.close_tab("b"));
        assert_eq!(group.tab_count(), 1);
    }

    #[test]
    fn dirty_and_close_saved() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.mark_dirty("a");
        assert_eq!(group.get_dirty_tabs().len(), 1);
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.tabs[0].id, "a");
    }

    #[test]
    fn pin_and_unpin() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("x"));
        group.pin_tab("x");
        assert!(group.tabs[0].pinned);
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 1);
        group.unpin_tab("x");
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 0);
    }

    #[test]
    fn move_tab_reorders() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        assert!(group.move_tab("a", 2));
        assert_eq!(group.get_tabs()[0].id, "b");
        assert_eq!(group.get_tabs()[2].id, "a");
    }

    #[test]
    fn move_tab_invalid_index() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        assert!(!group.move_tab("a", 5));
        assert!(!group.move_tab("missing", 0));
    }

    #[test]
    fn get_tab_returns_correct() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("x"));
        assert!(group.get_tab("x").is_some());
        assert!(group.get_tab("missing").is_none());
    }

    #[test]
    fn close_all_drains() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        let closed = group.close_all();
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tab_count(), 0);
    }

    #[test]
    fn close_others_keeps_target() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        let closed = group.close_others("b");
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.get_tabs()[0].id, "b");
    }

    #[test]
    fn close_to_right_and_left() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        group.add_tab(make_tab("d"));
        let right = group.close_to_right("b");
        assert_eq!(right.len(), 2);
        assert_eq!(group.tab_count(), 2);

        let left = group.close_to_left("b");
        assert_eq!(left.len(), 1);
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.get_tabs()[0].id, "b");
    }

    #[test]
    fn pinned_and_preview_tabs() {
        let mut group = TabGroup::new();
        let mut t1 = make_tab("a");
        t1.pinned = true;
        let mut t2 = make_tab("b");
        t2.preview = true;
        group.add_tab(t1);
        group.add_tab(t2);
        group.add_tab(make_tab("c"));
        assert_eq!(group.get_pinned_tabs().len(), 1);
        assert_eq!(group.get_preview_tabs().len(), 1);
        group.promote_preview("b");
        assert_eq!(group.get_preview_tabs().len(), 0);
    }

    #[test]
    fn find_by_uri_works() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab_with_uri("a", "file:///a.rs"));
        group.add_tab(make_tab("b"));
        assert_eq!(group.find_by_uri("file:///a.rs").unwrap().id, "a");
        assert!(group.find_by_uri("file:///missing").is_none());
    }

    #[test]
    fn tab_bar_config_defaults() {
        let config = TabBarConfig::default();
        assert!(config.show_icons);
        assert_eq!(config.tab_sizing, TabSizing::Fit);
        assert_eq!(config.close_button_position, CloseButtonPosition::Right);
    }

    #[test]
    fn tab_sizing_and_close_button_variants() {
        let _fit = TabSizing::Fit;
        let _fixed = TabSizing::Fixed;
        let _shrink = TabSizing::Shrink;
        let _left = CloseButtonPosition::Left;
        let _right = CloseButtonPosition::Right;
        let _off = CloseButtonPosition::Off;
        assert_ne!(TabSizing::Fit, TabSizing::Fixed);
        assert_ne!(CloseButtonPosition::Left, CloseButtonPosition::Off);
    }
}
