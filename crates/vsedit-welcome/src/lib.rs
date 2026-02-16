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

// ---------------------------------------------------------------------------
// WelcomeItemKind
// ---------------------------------------------------------------------------

/// The kind of a welcome item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WelcomeItemKind {
    Command,
    Link,
    Walkthrough,
    Extension,
}

// ---------------------------------------------------------------------------
// WalkthroughStep / WelcomeWalkthrough
// ---------------------------------------------------------------------------

/// A single step within a walkthrough.
#[derive(Debug, Clone)]
pub struct WalkthroughStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
}

/// A guided walkthrough shown on the welcome page.
#[derive(Debug, Clone)]
pub struct WelcomeWalkthrough {
    pub id: String,
    pub title: String,
    pub steps: Vec<WalkthroughStep>,
}

impl WelcomeWalkthrough {
    /// Returns `true` when every step has been completed.
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.completed)
    }

    /// Percentage of steps completed (0–100).
    pub fn completion_percentage(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let done = self.steps.iter().filter(|s| s.completed).count();
        (done as f64 / self.steps.len() as f64) * 100.0
    }

    /// Mark the step with the given `id` as completed.
    pub fn complete_step(&mut self, id: &str) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == id) {
            step.completed = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Extra WelcomePage methods
// ---------------------------------------------------------------------------

impl WelcomePage {
    /// Find a section by title.
    pub fn find_section(&self, title: &str) -> Option<&WelcomeSection> {
        self.sections.iter().find(|s| s.title == title)
    }

    /// Remove the first section with the given title. Returns `true` if removed.
    pub fn remove_section(&mut self, title: &str) -> bool {
        let before = self.sections.len();
        self.sections.retain(|s| s.title != title);
        self.sections.len() < before
    }

    /// Search all sections for an item with the given `id`.
    pub fn find_item_by_id(&self, id: &str) -> Option<&WelcomeItem> {
        self.sections
            .iter()
            .flat_map(|s| &s.items)
            .find(|item| item.id == id)
    }
}

// ---------------------------------------------------------------------------
// Extra RecentItemsList methods
// ---------------------------------------------------------------------------

impl RecentItemsList {
    /// Remove the first item whose `uri` matches.
    pub fn remove_item(&mut self, uri: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.uri != uri);
        self.items.len() < before
    }

    /// Returns `true` if any item has the given `uri`.
    pub fn contains(&self, uri: &str) -> bool {
        self.items.iter().any(|i| i.uri == uri)
    }
}

// ---------------------------------------------------------------------------
// WelcomeContentProvider trait
// ---------------------------------------------------------------------------

/// Trait for providing welcome-page content.
pub trait WelcomeContentProvider {
    /// Provide sections for the welcome page.
    fn provide_sections(&self) -> Vec<WelcomeSection> {
        Vec::new()
    }

    /// Provide walkthroughs for the welcome page.
    fn provide_walkthroughs(&self) -> Vec<WelcomeWalkthrough> {
        Vec::new()
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
    fn walkthrough_completion() {
        let mut wt = WelcomeWalkthrough {
            id: "wt1".to_string(),
            title: "Get Started".to_string(),
            steps: vec![
                WalkthroughStep {
                    id: "s1".to_string(),
                    title: "Step 1".to_string(),
                    description: "First step".to_string(),
                    completed: false,
                },
                WalkthroughStep {
                    id: "s2".to_string(),
                    title: "Step 2".to_string(),
                    description: "Second step".to_string(),
                    completed: false,
                },
            ],
        };
        assert!(!wt.is_complete());
        assert!((wt.completion_percentage() - 0.0).abs() < f64::EPSILON);
        wt.complete_step("s1");
        assert!((wt.completion_percentage() - 50.0).abs() < f64::EPSILON);
        wt.complete_step("s2");
        assert!(wt.is_complete());
        assert!((wt.completion_percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn walkthrough_complete_nonexistent_step() {
        let mut wt = WelcomeWalkthrough {
            id: "wt2".to_string(),
            title: "WT".to_string(),
            steps: vec![WalkthroughStep {
                id: "s1".to_string(),
                title: "S1".to_string(),
                description: "D".to_string(),
                completed: false,
            }],
        };
        wt.complete_step("nonexistent");
        assert!(!wt.is_complete());
    }

    #[test]
    fn walkthrough_empty_steps() {
        let wt = WelcomeWalkthrough {
            id: "wt3".to_string(),
            title: "Empty".to_string(),
            steps: vec![],
        };
        assert!(!wt.is_complete());
        assert!((wt.completion_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_section_by_title() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "Start".to_string(),
            items: vec![],
        });
        page.add_section(WelcomeSection {
            title: "Learn".to_string(),
            items: vec![],
        });
        assert!(page.find_section("Learn").is_some());
        assert!(page.find_section("Missing").is_none());
    }

    #[test]
    fn remove_section_by_title() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "Start".to_string(),
            items: vec![],
        });
        assert!(page.remove_section("Start"));
        assert_eq!(page.section_count(), 0);
        assert!(!page.remove_section("Start"));
    }

    #[test]
    fn find_item_by_id_across_sections() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "A".to_string(),
            items: vec![WelcomeItem {
                id: "a1".to_string(),
                title: "A1".to_string(),
                description: String::new(),
                icon: None,
                command: None,
            }],
        });
        page.add_section(WelcomeSection {
            title: "B".to_string(),
            items: vec![WelcomeItem {
                id: "b1".to_string(),
                title: "B1".to_string(),
                description: String::new(),
                icon: None,
                command: None,
            }],
        });
        assert_eq!(page.find_item_by_id("b1").unwrap().title, "B1");
        assert!(page.find_item_by_id("nope").is_none());
    }

    #[test]
    fn recent_items_remove_and_contains() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "a.rs".to_string(),
            label: "a".to_string(),
            timestamp: 1,
        });
        list.add(RecentItem {
            uri: "b.rs".to_string(),
            label: "b".to_string(),
            timestamp: 2,
        });
        assert!(list.contains("a.rs"));
        assert!(list.remove_item("a.rs"));
        assert!(!list.contains("a.rs"));
        assert!(!list.remove_item("a.rs"));
    }

    #[test]
    fn welcome_item_kind_variants() {
        let kinds = vec![
            WelcomeItemKind::Command,
            WelcomeItemKind::Link,
            WelcomeItemKind::Walkthrough,
            WelcomeItemKind::Extension,
        ];
        assert_eq!(kinds.len(), 4);
        assert_eq!(kinds[0], WelcomeItemKind::Command);
    }

    #[test]
    fn default_content_provider() {
        struct EmptyProvider;
        impl WelcomeContentProvider for EmptyProvider {}
        let p = EmptyProvider;
        assert!(p.provide_sections().is_empty());
        assert!(p.provide_walkthroughs().is_empty());
    }

    #[test]
    fn custom_content_provider() {
        struct MyProvider;
        impl WelcomeContentProvider for MyProvider {
            fn provide_sections(&self) -> Vec<WelcomeSection> {
                vec![WelcomeSection {
                    title: "Custom".to_string(),
                    items: vec![],
                }]
            }
        }
        let p = MyProvider;
        assert_eq!(p.provide_sections().len(), 1);
        assert!(p.provide_walkthroughs().is_empty());
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
