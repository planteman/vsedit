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
}

impl Default for TabGroup {
    fn default() -> Self {
        Self::new()
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
}
