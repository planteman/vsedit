//! Settings editor UI.
//!
//! Provides a settings editor with search, category navigation,
//! and type-appropriate value editors — rendered via ratatui.

use std::fmt;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// The data type of a setting value.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingType {
    Boolean,
    String,
    Number,
    Enum(Vec<String>),
    Array,
    Object,
}

/// The scope in which a setting applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsScope {
    User,
    Workspace,
    Folder,
}

/// A single setting entry.
#[derive(Debug, Clone)]
pub struct SettingEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub setting_type: SettingType,
    pub default_value: String,
    pub current_value: String,
    pub modified: bool,
}

impl SettingEntry {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        setting_type: SettingType,
        default_value: impl Into<String>,
    ) -> Self {
        let default = default_value.into();
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            category: category.into(),
            setting_type,
            current_value: default.clone(),
            default_value: default,
            modified: false,
        }
    }
}

// ---------------------------------------------------------------------------
// SettingsView
// ---------------------------------------------------------------------------

/// Settings editor view with search, categories, and value editing.
#[derive(Debug, Clone)]
pub struct SettingsView {
    pub entries: Vec<SettingEntry>,
    pub filtered_entries: Vec<usize>,
    pub search_query: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub active_scope: SettingsScope,
    pub categories: Vec<String>,
    pub active_category: Option<String>,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            filtered_entries: Vec::new(),
            search_query: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            active_scope: SettingsScope::User,
            categories: Vec::new(),
            active_category: None,
        }
    }

    /// Rebuild the categories list from entries.
    fn rebuild_categories(&mut self) {
        let mut cats: Vec<String> = self
            .entries
            .iter()
            .map(|e| e.category.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        self.categories = cats;
    }

    /// Add a setting entry.
    pub fn add_entry(&mut self, entry: SettingEntry) {
        self.entries.push(entry);
        self.rebuild_categories();
        self.refilter();
    }

    /// Filter entries by the current search query.
    pub fn filter_by_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.refilter();
    }

    /// Filter entries to a specific category.
    pub fn filter_by_category(&mut self, category: Option<String>) {
        self.active_category = category;
        self.refilter();
    }

    fn refilter(&mut self) {
        let lower_query = self.search_query.to_lowercase();
        self.filtered_entries = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if let Some(ref cat) = self.active_category {
                    if &e.category != cat {
                        return false;
                    }
                }
                if lower_query.is_empty() {
                    true
                } else {
                    e.title.to_lowercase().contains(&lower_query)
                        || e.id.to_lowercase().contains(&lower_query)
                        || e.description.to_lowercase().contains(&lower_query)
                }
            })
            .map(|(i, _)| i)
            .collect();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Toggle a boolean setting at the given filtered index.
    pub fn toggle_boolean(&mut self, filtered_index: usize) -> bool {
        if let Some(&entry_idx) = self.filtered_entries.get(filtered_index) {
            if let Some(entry) = self.entries.get_mut(entry_idx) {
                if entry.setting_type == SettingType::Boolean {
                    entry.current_value = if entry.current_value == "true" {
                        "false".to_string()
                    } else {
                        "true".to_string()
                    };
                    entry.modified = entry.current_value != entry.default_value;
                    return true;
                }
            }
        }
        false
    }

    /// Update a setting value at the given filtered index.
    pub fn update_value(&mut self, filtered_index: usize, value: impl Into<String>) -> bool {
        if let Some(&entry_idx) = self.filtered_entries.get(filtered_index) {
            if let Some(entry) = self.entries.get_mut(entry_idx) {
                entry.current_value = value.into();
                entry.modified = entry.current_value != entry.default_value;
                return true;
            }
        }
        false
    }

    /// Reset a setting to its default value.
    pub fn reset_to_default(&mut self, filtered_index: usize) -> bool {
        if let Some(&entry_idx) = self.filtered_entries.get(filtered_index) {
            if let Some(entry) = self.entries.get_mut(entry_idx) {
                entry.current_value = entry.default_value.clone();
                entry.modified = false;
                return true;
            }
        }
        false
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.filtered_entries.is_empty() {
            self.selected_index =
                (self.selected_index + 1).min(self.filtered_entries.len() - 1);
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Render the settings view.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 15 {
            return;
        }

        // Search bar (row 0).
        let search_area = Rect { height: 1, ..area };
        self.render_search_bar(search_area, buf);

        // Category nav (row 1).
        let cat_area = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
        self.render_category_nav(cat_area, buf);

        // Settings list (remaining rows).
        let list_area = Rect {
            y: area.y + 2,
            height: area.height.saturating_sub(2),
            ..area
        };
        self.render_settings_list(list_area, buf);
    }

    fn render_search_bar(&self, area: Rect, buf: &mut Buffer) {
        let label = if self.search_query.is_empty() {
            "🔍 Search settings...".to_string()
        } else {
            format!("🔍 {}", self.search_query)
        };
        let line = Line::from(vec![Span::styled(
            label,
            Style::default().fg(Color::Gray),
        )]);
        line.render(area, buf);
    }

    fn render_category_nav(&self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        // "All" option
        let is_all_active = self.active_category.is_none();
        let all_style = if is_all_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let all_label = " All ";
        let span = Span::styled(all_label, all_style);
        let line = Line::from(vec![span]);
        let r = Rect {
            x,
            y: area.y,
            width: all_label.len() as u16,
            height: 1,
        };
        line.render(r, buf);
        x += all_label.len() as u16;

        for cat in &self.categories {
            let is_active = self.active_category.as_ref() == Some(cat);
            let style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let label = format!(" {} ", cat);
            let width = label.len() as u16;
            if x + width > area.x + area.width {
                break;
            }
            let span = Span::styled(label, style);
            let line = Line::from(vec![span]);
            let r = Rect {
                x,
                y: area.y,
                width,
                height: 1,
            };
            line.render(r, buf);
            x += width;
        }
    }

    fn render_settings_list(&self, area: Rect, buf: &mut Buffer) {
        // Each setting takes 2 rows: title+value and description.
        let rows_per_entry = 2u16;
        let visible = (area.height / rows_per_entry) as usize;
        let start = self.scroll_offset;

        for (i, &entry_idx) in self
            .filtered_entries
            .iter()
            .skip(start)
            .take(visible)
            .enumerate()
        {
            let entry = &self.entries[entry_idx];
            let is_selected = start + i == self.selected_index;
            let y = area.y + (i as u16) * rows_per_entry;

            // Title + value line.
            let modified_marker = if entry.modified { "• " } else { "" };
            let value_preview = match &entry.setting_type {
                SettingType::Boolean => {
                    if entry.current_value == "true" {
                        "☑"
                    } else {
                        "☐"
                    }
                    .to_string()
                }
                SettingType::Enum(opts) => {
                    format!("[{}]", opts.join("|"))
                }
                _ => entry.current_value.clone(),
            };
            let title_text = format!("{}{}: {}", modified_marker, entry.title, value_preview);
            let title_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            let title_line = Line::from(vec![Span::styled(
                title_text.chars().take(area.width as usize).collect::<String>(),
                title_style,
            )]);
            let title_rect = Rect {
                y,
                height: 1,
                ..area
            };
            title_line.render(title_rect, buf);

            // Description line.
            if y + 1 < area.y + area.height {
                let desc_line = Line::from(vec![Span::styled(
                    format!("  {}", entry.description)
                        .chars()
                        .take(area.width as usize)
                        .collect::<String>(),
                    Style::default().fg(Color::DarkGray),
                )]);
                let desc_rect = Rect {
                    y: y + 1,
                    height: 1,
                    ..area
                };
                desc_line.render(desc_rect, buf);
            }
        }
    }
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Navigation helper
// ---------------------------------------------------------------------------

/// Keyboard navigation state for a settings list of known length.
#[derive(Debug, Clone)]
pub struct SettingsNavigation {
    selected: usize,
    total: usize,
    page_size: usize,
}

impl SettingsNavigation {
    /// Create a new navigation state.
    /// `total` is the number of items; `page_size` is used for page‑up/down.
    pub fn new(total: usize, page_size: usize) -> Self {
        Self {
            selected: 0,
            total,
            page_size: page_size.max(1),
        }
    }

    pub fn move_down(&mut self) {
        if self.total > 0 {
            self.selected = (self.selected + 1).min(self.total - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        if self.total > 0 {
            self.selected = (self.selected + self.page_size).min(self.total - 1);
        }
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.page_size);
    }

    pub fn go_to_first(&mut self) {
        self.selected = 0;
    }

    pub fn go_to_last(&mut self) {
        if self.total > 0 {
            self.selected = self.total - 1;
        }
    }

    pub fn get_selected_index(&self) -> usize {
        self.selected
    }

    /// Update the total item count, clamping the selection if needed.
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        if self.total == 0 {
            self.selected = 0;
        } else if self.selected >= self.total {
            self.selected = self.total - 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Search state
// ---------------------------------------------------------------------------

/// Tracks the current search query and the indices that matched.
#[derive(Debug, Clone)]
pub struct SettingsSearchState {
    pub query: String,
    pub filtered_indices: Vec<usize>,
}

impl SettingsSearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            filtered_indices: Vec::new(),
        }
    }

    /// Run a case-insensitive search over the given entries, storing matches.
    pub fn search(&mut self, query: &str, entries: &[SettingEntry]) {
        self.query = query.to_string();
        let lower = query.to_lowercase();
        if lower.is_empty() {
            self.filtered_indices = (0..entries.len()).collect();
        } else {
            self.filtered_indices = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.title.to_lowercase().contains(&lower)
                        || e.id.to_lowercase().contains(&lower)
                        || e.description.to_lowercase().contains(&lower)
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub fn result_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }
}

impl Default for SettingsSearchState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb path
// ---------------------------------------------------------------------------

/// Tracks the category hierarchy the user has navigated into
/// (e.g. "Editor" → "Font" → "Ligatures").
#[derive(Debug, Clone)]
pub struct BreadcrumbPath {
    segments: Vec<String>,
}

impl BreadcrumbPath {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Push a new segment onto the path.
    pub fn push(&mut self, segment: impl Into<String>) {
        self.segments.push(segment.into());
    }

    /// Pop the last segment, returning it if present.
    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    /// The most specific (deepest) segment, or `None` when at root.
    pub fn current(&self) -> Option<&str> {
        self.segments.last().map(|s| s.as_str())
    }

    /// Number of segments in the path.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// `true` when no segments have been pushed.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }
}

impl std::fmt::Display for BreadcrumbPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.segments.is_empty() {
            write!(f, "Settings")
        } else {
            write!(f, "Settings > {}", self.segments.join(" > "))
        }
    }
}

impl Default for BreadcrumbPath {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Validation helper
// ---------------------------------------------------------------------------

/// Validates that a `SettingsView` is in a consistent state:
///
/// * Every index in `filtered_entries` is within bounds of `entries`.
/// * `selected_index` is within bounds of `filtered_entries` (or both are 0).
/// * `active_category`, if set, appears in `categories`.
///
/// Returns a list of human-readable error strings (empty = valid).
pub fn validate_settings_view_state(view: &SettingsView) -> Vec<String> {
    let mut errors = Vec::new();

    for (pos, &idx) in view.filtered_entries.iter().enumerate() {
        if idx >= view.entries.len() {
            errors.push(format!(
                "filtered_entries[{}] = {} is out of bounds (entries len = {})",
                pos,
                idx,
                view.entries.len()
            ));
        }
    }

    if !view.filtered_entries.is_empty() && view.selected_index >= view.filtered_entries.len() {
        errors.push(format!(
            "selected_index {} is out of bounds (filtered len = {})",
            view.selected_index,
            view.filtered_entries.len()
        ));
    }

    if let Some(ref cat) = view.active_category {
        if !view.categories.contains(cat) {
            errors.push(format!(
                "active_category {:?} not found in categories list",
                cat
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<SettingEntry> {
        vec![
            SettingEntry::new(
                "editor.fontSize",
                "Font Size",
                "Controls the font size in pixels",
                "Editor",
                SettingType::Number,
                "14",
            ),
            SettingEntry::new(
                "editor.wordWrap",
                "Word Wrap",
                "Controls how lines should wrap",
                "Editor",
                SettingType::Enum(vec!["off".into(), "on".into(), "bounded".into()]),
                "off",
            ),
            SettingEntry::new(
                "editor.minimap.enabled",
                "Minimap Enabled",
                "Show minimap",
                "Editor",
                SettingType::Boolean,
                "true",
            ),
            SettingEntry::new(
                "terminal.fontFamily",
                "Terminal Font",
                "Font family for the terminal",
                "Terminal",
                SettingType::String,
                "monospace",
            ),
        ]
    }

    #[test]
    fn creation() {
        let v = SettingsView::new();
        assert!(v.entries.is_empty());
        assert_eq!(v.active_scope, SettingsScope::User);
    }

    #[test]
    fn add_entry_builds_categories() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        assert!(v.categories.contains(&"Editor".to_string()));
        assert!(v.categories.contains(&"Terminal".to_string()));
    }

    #[test]
    fn filter_by_query() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.filter_by_query("font");
        assert_eq!(v.filtered_entries.len(), 2); // fontSize + fontFamily
    }

    #[test]
    fn filter_by_category() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.filter_by_category(Some("Terminal".to_string()));
        assert_eq!(v.filtered_entries.len(), 1);
    }

    #[test]
    fn toggle_boolean() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        // minimap.enabled is at index 2 in entries, find it in filtered
        let pos = v
            .filtered_entries
            .iter()
            .position(|&i| v.entries[i].id == "editor.minimap.enabled")
            .unwrap();
        assert!(v.toggle_boolean(pos));
        let entry_idx = v.filtered_entries[pos];
        assert_eq!(v.entries[entry_idx].current_value, "false");
        assert!(v.entries[entry_idx].modified);
    }

    #[test]
    fn update_value() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        assert!(v.update_value(0, "16"));
        let entry_idx = v.filtered_entries[0];
        assert_eq!(v.entries[entry_idx].current_value, "16");
        assert!(v.entries[entry_idx].modified);
    }

    #[test]
    fn reset_to_default() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.update_value(0, "20");
        v.reset_to_default(0);
        let entry_idx = v.filtered_entries[0];
        assert_eq!(v.entries[entry_idx].current_value, "14");
        assert!(!v.entries[entry_idx].modified);
    }

    #[test]
    fn select_next_and_previous() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.select_next();
        assert_eq!(v.selected_index, 1);
        v.select_previous();
        assert_eq!(v.selected_index, 0);
    }

    #[test]
    fn render_does_not_panic() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_empty_no_panic() {
        let v = SettingsView::new();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let v = SettingsView::default();
        assert!(v.entries.is_empty());
    }

    // --- SettingsNavigation tests ---

    #[test]
    fn navigation_move_and_bounds() {
        let mut nav = SettingsNavigation::new(5, 3);
        assert_eq!(nav.get_selected_index(), 0);
        nav.move_down();
        nav.move_down();
        assert_eq!(nav.get_selected_index(), 2);
        nav.move_up();
        assert_eq!(nav.get_selected_index(), 1);
        // Cannot go below 0
        nav.go_to_first();
        nav.move_up();
        assert_eq!(nav.get_selected_index(), 0);
        // Cannot exceed total-1
        nav.go_to_last();
        assert_eq!(nav.get_selected_index(), 4);
        nav.move_down();
        assert_eq!(nav.get_selected_index(), 4);
    }

    #[test]
    fn navigation_page_up_down() {
        let mut nav = SettingsNavigation::new(20, 5);
        nav.page_down();
        assert_eq!(nav.get_selected_index(), 5);
        nav.page_down();
        assert_eq!(nav.get_selected_index(), 10);
        nav.page_up();
        assert_eq!(nav.get_selected_index(), 5);
        nav.page_up();
        assert_eq!(nav.get_selected_index(), 0);
        // Clamp at end
        nav.go_to_last();
        nav.page_down();
        assert_eq!(nav.get_selected_index(), 19);
    }

    #[test]
    fn navigation_set_total_clamps() {
        let mut nav = SettingsNavigation::new(10, 3);
        nav.go_to_last();
        assert_eq!(nav.get_selected_index(), 9);
        nav.set_total(5);
        assert_eq!(nav.get_selected_index(), 4);
        nav.set_total(0);
        assert_eq!(nav.get_selected_index(), 0);
    }

    // --- SettingsSearchState tests ---

    #[test]
    fn search_state_filters_entries() {
        let entries = sample_entries();
        let mut ss = SettingsSearchState::new();
        assert!(!ss.is_active());
        ss.search("font", &entries);
        assert!(ss.is_active());
        assert_eq!(ss.result_count(), 2); // fontSize + fontFamily
        ss.search("", &entries);
        assert_eq!(ss.result_count(), 4);
    }

    // --- BreadcrumbPath tests ---

    #[test]
    fn breadcrumb_push_pop_display() {
        let mut bc = BreadcrumbPath::new();
        assert!(bc.is_root());
        assert_eq!(bc.to_string(), "Settings");
        bc.push("Editor");
        assert_eq!(bc.current(), Some("Editor"));
        assert_eq!(bc.depth(), 1);
        bc.push("Font");
        assert_eq!(bc.to_string(), "Settings > Editor > Font");
        assert_eq!(bc.pop(), Some("Font".to_string()));
        assert_eq!(bc.depth(), 1);
        assert_eq!(bc.pop(), Some("Editor".to_string()));
        assert!(bc.is_root());
    }

    // --- validate_settings_view_state tests ---

    #[test]
    fn validate_view_state_ok() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        let errors = validate_settings_view_state(&v);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn validate_view_state_bad_category() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.active_category = Some("NonExistent".to_string());
        let errors = validate_settings_view_state(&v);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("NonExistent"));
    }
}
