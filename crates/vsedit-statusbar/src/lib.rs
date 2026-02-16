//! Status bar widget.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAlignment {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct StatusBarEntry {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub alignment: StatusBarAlignment,
    pub priority: i32,
    pub visible: bool,
    pub color: Option<String>,
    pub background_color: Option<String>,
}

pub struct StatusBar {
    entries: Vec<StatusBarEntry>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: StatusBarEntry) {
        self.entries.push(entry);
    }

    pub fn remove_entry(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != len
    }

    pub fn update_text(&mut self, id: &str, text: impl Into<String>) {
        let text = text.into();
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.text = text;
        }
    }

    pub fn get_visible_entries(&self, alignment: StatusBarAlignment) -> Vec<&StatusBarEntry> {
        let mut entries: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == alignment)
            .collect();
        entries.sort_by_key(|e| e.priority);
        entries
    }

    pub fn set_visibility(&mut self, id: &str, visible: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.visible = visible;
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, alignment: StatusBarAlignment, priority: i32) -> StatusBarEntry {
        StatusBarEntry {
            id: id.to_string(),
            text: id.to_string(),
            tooltip: None,
            command: None,
            alignment,
            priority,
            visible: true,
            color: None,
            background_color: None,
        }
    }

    #[test]
    fn add_and_remove() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("git", StatusBarAlignment::Left, 0));
        assert_eq!(bar.entry_count(), 1);
        assert!(bar.remove_entry("git"));
        assert!(!bar.remove_entry("git"));
        assert_eq!(bar.entry_count(), 0);
    }

    #[test]
    fn visible_entries_sorted() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 10));
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("r", StatusBarAlignment::Right, 5));
        let left = bar.get_visible_entries(StatusBarAlignment::Left);
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].id, "a");
        assert_eq!(left[1].id, "b");
        assert_eq!(bar.get_visible_entries(StatusBarAlignment::Right).len(), 1);
    }

    #[test]
    fn update_text_and_visibility() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("info", StatusBarAlignment::Left, 0));
        bar.update_text("info", "updated");
        bar.set_visibility("info", false);
        assert_eq!(bar.get_visible_entries(StatusBarAlignment::Left).len(), 0);
    }
}
