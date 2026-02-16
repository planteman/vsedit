//! Welcome page.

/// A single item on the welcome page.
#[derive(Debug, Clone)]
pub struct WelcomeItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: Option<String>,
    pub command: Option<String>,
}

/// A section grouping related welcome items.
#[derive(Debug, Clone)]
pub struct WelcomeSection {
    pub title: String,
    pub items: Vec<WelcomeItem>,
}

/// The welcome page shown on startup.
pub struct WelcomePage {
    pub sections: Vec<WelcomeSection>,
    pub show_on_startup: bool,
}

impl WelcomePage {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            show_on_startup: true,
        }
    }

    pub fn add_section(&mut self, section: WelcomeSection) {
        self.sections.push(section);
    }

    pub fn set_show_on_startup(&mut self, show: bool) {
        self.show_on_startup = show;
    }

    pub fn should_show(&self) -> bool {
        self.show_on_startup
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn total_item_count(&self) -> usize {
        self.sections.iter().map(|s| s.items.len()).sum()
    }
}

impl Default for WelcomePage {
    fn default() -> Self {
        Self::new()
    }
}

/// A recently opened item.
#[derive(Debug, Clone)]
pub struct RecentItem {
    pub uri: String,
    pub label: String,
    pub timestamp: u64,
}

/// List of recently opened items, sorted by most recent first.
pub struct RecentItemsList {
    items: Vec<RecentItem>,
}

impl RecentItemsList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: RecentItem) {
        self.items.push(item);
        self.items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    pub fn get_recent(&self, max: usize) -> &[RecentItem] {
        let end = max.min(self.items.len());
        &self.items[..end]
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl Default for RecentItemsList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_page_sections() {
        let mut page = WelcomePage::new();
        assert!(page.should_show());
        let section = WelcomeSection {
            title: "Start".to_string(),
            items: vec![
                WelcomeItem {
                    id: "new-file".to_string(),
                    title: "New File".to_string(),
                    description: "Create a new file".to_string(),
                    icon: None,
                    command: Some("newFile".to_string()),
                },
            ],
        };
        page.add_section(section);
        assert_eq!(page.section_count(), 1);
        assert_eq!(page.total_item_count(), 1);
    }

    #[test]
    fn welcome_page_show_toggle() {
        let mut page = WelcomePage::new();
        page.set_show_on_startup(false);
        assert!(!page.should_show());
    }

    #[test]
    fn recent_items_ordering() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "old.rs".to_string(),
            label: "old".to_string(),
            timestamp: 100,
        });
        list.add(RecentItem {
            uri: "new.rs".to_string(),
            label: "new".to_string(),
            timestamp: 200,
        });
        let recent = list.get_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].uri, "new.rs");
    }

    #[test]
    fn recent_items_clear() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "f.rs".to_string(),
            label: "f".to_string(),
            timestamp: 1,
        });
        list.clear();
        assert_eq!(list.get_recent(10).len(), 0);
    }
}
