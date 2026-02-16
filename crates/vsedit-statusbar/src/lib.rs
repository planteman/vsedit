//! Status bar widget.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAlignment {
    Left,
    Right,
}

impl fmt::Display for StatusBarAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarAlignment::Left => write!(f, "Left"),
            StatusBarAlignment::Right => write!(f, "Right"),
        }
    }
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

impl StatusBarEntry {
    pub fn builder(
        id: impl Into<String>,
        text: impl Into<String>,
        alignment: StatusBarAlignment,
    ) -> StatusBarEntryBuilder {
        StatusBarEntryBuilder {
            id: id.into(),
            text: text.into(),
            alignment,
            tooltip: None,
            command: None,
            priority: 0,
            color: None,
            background_color: None,
            visible: true,
        }
    }
}

impl fmt::Display for StatusBarEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.alignment, self.text)
    }
}

pub struct StatusBarEntryBuilder {
    id: String,
    text: String,
    alignment: StatusBarAlignment,
    tooltip: Option<String>,
    command: Option<String>,
    priority: i32,
    color: Option<String>,
    background_color: Option<String>,
    visible: bool,
}

impl StatusBarEntryBuilder {
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn background_color(mut self, color: impl Into<String>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn build(self) -> StatusBarEntry {
        StatusBarEntry {
            id: self.id,
            text: self.text,
            tooltip: self.tooltip,
            command: self.command,
            alignment: self.alignment,
            priority: self.priority,
            visible: self.visible,
            color: self.color,
            background_color: self.background_color,
        }
    }
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

    pub fn get_entry(&self, id: &str) -> Option<&StatusBarEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn update_tooltip(&mut self, id: &str, tooltip: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.tooltip = Some(tooltip.to_string());
        }
    }

    pub fn update_color(&mut self, id: &str, color: Option<String>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.color = color;
        }
    }

    pub fn update_background_color(&mut self, id: &str, color: Option<String>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.background_color = color;
        }
    }

    pub fn get_all_entries(&self) -> &[StatusBarEntry] {
        &self.entries
    }

    pub fn has_entry(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn visible_count(&self) -> usize {
        self.entries.iter().filter(|e| e.visible).count()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
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

    #[test]
    fn builder_pattern() {
        let entry = StatusBarEntry::builder("git", "main", StatusBarAlignment::Left)
            .tooltip("Current branch")
            .command("git.checkout")
            .priority(5)
            .color("#fff")
            .background_color("#000")
            .visible(false)
            .build();
        assert_eq!(entry.id, "git");
        assert_eq!(entry.text, "main");
        assert_eq!(entry.tooltip.as_deref(), Some("Current branch"));
        assert_eq!(entry.command.as_deref(), Some("git.checkout"));
        assert_eq!(entry.priority, 5);
        assert_eq!(entry.color.as_deref(), Some("#fff"));
        assert_eq!(entry.background_color.as_deref(), Some("#000"));
        assert!(!entry.visible);
    }

    #[test]
    fn builder_defaults() {
        let entry = StatusBarEntry::builder("id", "text", StatusBarAlignment::Right).build();
        assert!(entry.visible);
        assert_eq!(entry.priority, 0);
        assert!(entry.tooltip.is_none());
        assert!(entry.command.is_none());
        assert!(entry.color.is_none());
        assert!(entry.background_color.is_none());
    }

    #[test]
    fn get_entry_and_has_entry() {
        let mut bar = StatusBar::new();
        assert!(!bar.has_entry("x"));
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        assert!(bar.has_entry("x"));
        let e = bar.get_entry("x").unwrap();
        assert_eq!(e.id, "x");
        assert!(bar.get_entry("missing").is_none());
    }

    #[test]
    fn update_tooltip_and_colors() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("s", StatusBarAlignment::Right, 0));
        bar.update_tooltip("s", "hello");
        bar.update_color("s", Some("#red".to_string()));
        bar.update_background_color("s", Some("#blue".to_string()));
        let e = bar.get_entry("s").unwrap();
        assert_eq!(e.tooltip.as_deref(), Some("hello"));
        assert_eq!(e.color.as_deref(), Some("#red"));
        assert_eq!(e.background_color.as_deref(), Some("#blue"));
        bar.update_color("s", None);
        bar.update_background_color("s", None);
        let e = bar.get_entry("s").unwrap();
        assert!(e.color.is_none());
        assert!(e.background_color.is_none());
    }

    #[test]
    fn visible_count_and_clear() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 1));
        bar.set_visibility("b", false);
        assert_eq!(bar.visible_count(), 1);
        assert_eq!(bar.get_all_entries().len(), 2);
        bar.clear();
        assert_eq!(bar.entry_count(), 0);
        assert_eq!(bar.visible_count(), 0);
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", StatusBarAlignment::Left), "Left");
        assert_eq!(format!("{}", StatusBarAlignment::Right), "Right");
        let entry = StatusBarEntry::builder("id", "hello", StatusBarAlignment::Right).build();
        assert_eq!(format!("{}", entry), "[Right] hello");
    }
}
