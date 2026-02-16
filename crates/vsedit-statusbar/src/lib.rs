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

// --- New features ---

impl StatusBar {
    /// Return entries whose text contains the given substring.
    pub fn find_entries(&self, substring: &str) -> Vec<&StatusBarEntry> {
        self.entries
            .iter()
            .filter(|e| e.text.contains(substring))
            .collect()
    }

    /// Sort all entries in-place by priority (ascending).
    pub fn sort_by_priority(&mut self) {
        self.entries.sort_by_key(|e| e.priority);
    }

    /// Toggle the visibility of an entry. Returns `true` if the entry was found.
    pub fn toggle_visibility(&mut self, id: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.visible = !entry.visible;
            true
        } else {
            false
        }
    }

    /// Render a formatted string of visible left-aligned entries separated by spaces,
    /// sorted by priority.
    pub fn render_left_text(&self) -> String {
        let mut left: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == StatusBarAlignment::Left)
            .collect();
        left.sort_by_key(|e| e.priority);
        left.iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Render a formatted string of visible right-aligned entries separated by spaces,
    /// sorted by priority.
    pub fn render_right_text(&self) -> String {
        let mut right: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == StatusBarAlignment::Right)
            .collect();
        right.sort_by_key(|e| e.priority);
        right
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Move an entry to a different alignment. Returns `true` if the entry was found.
    pub fn move_entry(&mut self, id: &str, alignment: StatusBarAlignment) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.alignment = alignment;
            true
        } else {
            false
        }
    }

    /// Bulk-update fields of an entry via a callback closure.
    /// Returns `true` if the entry was found and the callback was applied.
    pub fn update_entry<F>(&mut self, id: &str, f: F) -> bool
    where
        F: FnOnce(&mut StatusBarEntry),
    {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            f(entry);
            true
        } else {
            false
        }
    }

    /// Return all entries that have a command set.
    pub fn entries_with_command(&self) -> Vec<&StatusBarEntry> {
        self.entries
            .iter()
            .filter(|e| e.command.is_some())
            .collect()
    }

    /// Capture a snapshot of the current status bar state.
    pub fn snapshot(&self) -> StatusBarSnapshot {
        StatusBarSnapshot {
            entries: self.entries.clone(),
        }
    }

    /// Restore the status bar from a previously captured snapshot.
    pub fn restore(&mut self, snapshot: &StatusBarSnapshot) {
        self.entries = snapshot.entries.clone();
    }

    /// Merge entries from another `StatusBar`, skipping entries whose id already exists.
    pub fn merge(&mut self, other: &StatusBar) {
        for entry in &other.entries {
            if !self.has_entry(&entry.id) {
                self.entries.push(entry.clone());
            }
        }
    }

    /// Reorder entries according to the given list of IDs.
    /// IDs present in the list are placed first (in the given order),
    /// followed by any remaining entries in their original order.
    pub fn reorder(&mut self, ids: &[&str]) {
        let mut ordered: Vec<StatusBarEntry> = Vec::with_capacity(self.entries.len());
        for &id in ids {
            if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
                ordered.push(self.entries.remove(pos));
            }
        }
        ordered.append(&mut self.entries);
        self.entries = ordered;
    }
}

/// A snapshot of a `StatusBar`'s entries that can be used to restore state.
#[derive(Debug, Clone)]
pub struct StatusBarSnapshot {
    entries: Vec<StatusBarEntry>,
}

impl StatusBarSnapshot {
    /// Number of entries captured in this snapshot.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get a reference to a captured entry by id.
    pub fn get_entry(&self, id: &str) -> Option<&StatusBarEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get all captured entries.
    pub fn entries(&self) -> &[StatusBarEntry] {
        &self.entries
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

    // --- New tests ---

    #[test]
    fn find_entries_by_substring() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("branch", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("errors", StatusBarAlignment::Left, 1));
        bar.update_text("branch", "main branch");
        bar.update_text("errors", "0 errors");
        let found = bar.find_entries("branch");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "branch");
    }

    #[test]
    fn find_entries_no_match() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        assert!(bar.find_entries("zzz").is_empty());
    }

    #[test]
    fn sort_by_priority_reorders() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("c", StatusBarAlignment::Left, 30));
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 10));
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 20));
        bar.sort_by_priority();
        let ids: Vec<&str> = bar.get_all_entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn toggle_visibility_flips() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        assert!(bar.get_entry("x").unwrap().visible);
        assert!(bar.toggle_visibility("x"));
        assert!(!bar.get_entry("x").unwrap().visible);
        assert!(bar.toggle_visibility("x"));
        assert!(bar.get_entry("x").unwrap().visible);
    }

    #[test]
    fn toggle_visibility_missing() {
        let mut bar = StatusBar::new();
        assert!(!bar.toggle_visibility("nope"));
    }

    #[test]
    fn render_left_text_sorted() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 10));
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("r", StatusBarAlignment::Right, 5));
        bar.update_text("b", "second");
        bar.update_text("a", "first");
        assert_eq!(bar.render_left_text(), "first second");
    }

    #[test]
    fn render_right_text_sorted() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("r2", StatusBarAlignment::Right, 20));
        bar.add_entry(make_entry("r1", StatusBarAlignment::Right, 5));
        bar.update_text("r1", "alpha");
        bar.update_text("r2", "beta");
        assert_eq!(bar.render_right_text(), "alpha beta");
    }

    #[test]
    fn render_text_excludes_hidden() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("v", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("h", StatusBarAlignment::Left, 1));
        bar.set_visibility("h", false);
        bar.update_text("v", "visible");
        bar.update_text("h", "hidden");
        assert_eq!(bar.render_left_text(), "visible");
    }

    #[test]
    fn render_text_empty() {
        let bar = StatusBar::new();
        assert_eq!(bar.render_left_text(), "");
        assert_eq!(bar.render_right_text(), "");
    }

    #[test]
    fn move_entry_changes_alignment() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("m", StatusBarAlignment::Left, 0));
        assert!(bar.move_entry("m", StatusBarAlignment::Right));
        assert_eq!(bar.get_entry("m").unwrap().alignment, StatusBarAlignment::Right);
        assert!(!bar.move_entry("missing", StatusBarAlignment::Left));
    }

    #[test]
    fn update_entry_bulk() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("u", StatusBarAlignment::Left, 0));
        let found = bar.update_entry("u", |e| {
            e.text = "new text".to_string();
            e.priority = 99;
            e.color = Some("#abc".to_string());
            e.visible = false;
        });
        assert!(found);
        let e = bar.get_entry("u").unwrap();
        assert_eq!(e.text, "new text");
        assert_eq!(e.priority, 99);
        assert_eq!(e.color.as_deref(), Some("#abc"));
        assert!(!e.visible);
    }

    #[test]
    fn update_entry_missing() {
        let mut bar = StatusBar::new();
        assert!(!bar.update_entry("nope", |_| {}));
    }

    #[test]
    fn entries_with_command_filters() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("cmd1", "text", StatusBarAlignment::Left)
                .command("do.thing")
                .build(),
        );
        bar.add_entry(make_entry("no_cmd", StatusBarAlignment::Left, 0));
        bar.add_entry(
            StatusBarEntry::builder("cmd2", "text2", StatusBarAlignment::Right)
                .command("do.other")
                .build(),
        );
        let with_cmd = bar.entries_with_command();
        assert_eq!(with_cmd.len(), 2);
        assert!(with_cmd.iter().all(|e| e.command.is_some()));
    }

    #[test]
    fn snapshot_and_restore() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 1));

        let snap = bar.snapshot();
        assert_eq!(snap.entry_count(), 2);
        assert!(snap.get_entry("a").is_some());
        assert_eq!(snap.entries().len(), 2);

        bar.clear();
        assert_eq!(bar.entry_count(), 0);

        bar.restore(&snap);
        assert_eq!(bar.entry_count(), 2);
        assert_eq!(bar.get_entry("a").unwrap().text, "a");
    }

    #[test]
    fn snapshot_is_independent() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        let snap = bar.snapshot();
        bar.update_text("x", "changed");
        assert_eq!(snap.get_entry("x").unwrap().text, "x");
    }

    #[test]
    fn merge_skips_duplicates() {
        let mut bar1 = StatusBar::new();
        bar1.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar1.add_entry(make_entry("b", StatusBarAlignment::Left, 1));

        let mut bar2 = StatusBar::new();
        bar2.add_entry(make_entry("b", StatusBarAlignment::Right, 99));
        bar2.add_entry(make_entry("c", StatusBarAlignment::Right, 2));

        bar1.merge(&bar2);
        assert_eq!(bar1.entry_count(), 3);
        // "b" should keep original alignment (Left) since it was a duplicate
        assert_eq!(bar1.get_entry("b").unwrap().alignment, StatusBarAlignment::Left);
        assert!(bar1.has_entry("c"));
    }

    #[test]
    fn merge_empty_into_empty() {
        let mut bar1 = StatusBar::new();
        let bar2 = StatusBar::new();
        bar1.merge(&bar2);
        assert_eq!(bar1.entry_count(), 0);
    }

    #[test]
    fn reorder_by_ids() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("c", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 2));

        bar.reorder(&["b", "a", "c"]);
        let ids: Vec<&str> = bar.get_all_entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
    }

    #[test]
    fn reorder_partial_ids() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("y", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("z", StatusBarAlignment::Left, 2));

        bar.reorder(&["z"]);
        let ids: Vec<&str> = bar.get_all_entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["z", "x", "y"]);
    }

    #[test]
    fn reorder_with_unknown_ids() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 1));

        bar.reorder(&["missing", "b", "a"]);
        let ids: Vec<&str> = bar.get_all_entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }
}
