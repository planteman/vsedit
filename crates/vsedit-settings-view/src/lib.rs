//! Settings editor UI.
//!
//! Provides a settings editor with search, category navigation,
//! and type-appropriate value editors — rendered via ratatui.

use std::collections::HashMap;
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

/// Represents a navigation breadcrumb path in the settings UI (e.g. "User > Editor > Font").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsBreadcrumb {
    segments: Vec<String>,
}

impl SettingsBreadcrumb {
    /// Create an empty breadcrumb.
    pub fn new() -> Self {
        Self { segments: Vec::new() }
    }

    /// Push a new segment onto the breadcrumb trail.
    pub fn push(&mut self, segment: &str) {
        self.segments.push(segment.to_string());
    }

    /// Pop the last segment, returning it if present.
    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    /// Return the display string with segments joined by " > ".
    pub fn display(&self) -> String {
        self.segments.join(" > ")
    }

    /// Return the current depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return true if the breadcrumb is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl Default for SettingsBreadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SettingsTreeNode – hierarchical settings display
// ---------------------------------------------------------------------------

/// A tree node for hierarchical settings navigation.
#[derive(Debug, Clone)]
pub struct SettingsTreeNode {
    pub key: String,
    pub label: String,
    pub children: Vec<SettingsTreeNode>,
    pub entry_index: Option<usize>,
    pub expanded: bool,
}

impl SettingsTreeNode {
    /// Create a new tree node with the given key and label.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            children: Vec::new(),
            entry_index: None,
            expanded: false,
        }
    }

    /// Append a child node and return a mutable reference to it.
    pub fn add_child(&mut self, child: SettingsTreeNode) -> &mut SettingsTreeNode {
        self.children.push(child);
        self.children.last_mut().unwrap()
    }

    /// A leaf node has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Toggle the expanded state.
    pub fn toggle_expand(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Recursively search for a node by key, returning a reference if found.
    pub fn find_by_key(&self, key: &str) -> Option<&SettingsTreeNode> {
        if self.key == key {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_key(key) {
                return Some(found);
            }
        }
        None
    }
}

/// Build a tree from a slice of `SettingEntry` values using dotted IDs.
///
/// For example, `editor.fontSize` produces root → editor → fontSize.
pub fn build_tree(entries: &[SettingEntry]) -> SettingsTreeNode {
    let mut root = SettingsTreeNode::new("root", "Settings");
    root.expanded = true;

    for (idx, entry) in entries.iter().enumerate() {
        let parts: Vec<&str> = entry.id.split('.').collect();
        let mut current = &mut root;

        for (depth, part) in parts.iter().enumerate() {
            let is_last = depth == parts.len() - 1;
            let pos = current.children.iter().position(|c| c.key == *part);

            if let Some(pos) = pos {
                current = &mut current.children[pos];
            } else {
                let label = if is_last {
                    entry.title.clone()
                } else {
                    (*part).to_string()
                };
                let mut node = SettingsTreeNode::new(*part, label);
                if is_last {
                    node.entry_index = Some(idx);
                }
                current.add_child(node);
                let last = current.children.len() - 1;
                current = &mut current.children[last];
            }
        }
    }

    root
}

// ---------------------------------------------------------------------------
// SettingsSearchIndex – fast keyword search
// ---------------------------------------------------------------------------

/// Pre-built search index for fast case-insensitive keyword matching.
#[derive(Debug, Clone)]
pub struct SettingsSearchIndex {
    terms: Vec<(usize, Vec<String>)>,
}

impl SettingsSearchIndex {
    /// Build a search index from a slice of setting entries.
    pub fn build(entries: &[SettingEntry]) -> Self {
        let terms = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mut tokens: Vec<String> = Vec::new();
                tokens.push(e.id.to_lowercase());
                tokens.push(e.title.to_lowercase());
                tokens.push(e.description.to_lowercase());
                tokens.push(e.category.to_lowercase());
                (i, tokens)
            })
            .collect();
        Self { terms }
    }

    /// Return indices of entries whose searchable terms contain all query words
    /// (case-insensitive, AND semantics).
    pub fn search(&self, query: &str) -> Vec<usize> {
        let words: Vec<String> = query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        if words.is_empty() {
            return (0..self.terms.len()).collect();
        }

        self.terms
            .iter()
            .filter(|(_, tokens)| {
                words.iter().all(|w| tokens.iter().any(|t| t.contains(w.as_str())))
            })
            .map(|(i, _)| *i)
            .collect()
    }

    /// Number of entries in the index.
    pub fn entry_count(&self) -> usize {
        self.terms.len()
    }
}

// ---------------------------------------------------------------------------
// SettingsModifiedFilter
// ---------------------------------------------------------------------------

/// Return indices of entries whose current value differs from the default.
pub fn filter_modified(entries: &[SettingEntry]) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.modified || e.current_value != e.default_value)
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// SettingsSnapshot – capture a point-in-time state
// ---------------------------------------------------------------------------

/// A frozen snapshot of setting IDs to their values at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSnapshot {
    /// Pairs of `(setting_id, value)` sorted by id.
    entries: Vec<(String, String)>,
    pub label: String,
}

impl SettingsSnapshot {
    /// Capture the current values from a slice of entries.
    pub fn capture(entries: &[SettingEntry], label: impl Into<String>) -> Self {
        let mut pairs: Vec<(String, String)> = entries
            .iter()
            .map(|e| (e.id.clone(), e.current_value.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            entries: pairs,
            label: label.into(),
        }
    }

    /// Look up the value for a given setting id.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.entries
            .binary_search_by_key(&id, |(k, _)| k.as_str())
            .ok()
            .map(|i| self.entries[i].1.as_str())
    }

    /// Number of settings in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SettingsDiff – compare two snapshots
// ---------------------------------------------------------------------------

/// The kind of change between two snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    Added,
    Removed,
    Changed { old: String, new: String },
}

/// A single difference between two snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDiffEntry {
    pub id: String,
    pub kind: DiffKind,
}

/// Compute the differences between two snapshots.
pub fn diff_snapshots(before: &SettingsSnapshot, after: &SettingsSnapshot) -> Vec<SettingsDiffEntry> {
    let mut diffs = Vec::new();
    let mut bi = 0;
    let mut ai = 0;

    while bi < before.entries.len() && ai < after.entries.len() {
        let (ref bk, ref bv) = before.entries[bi];
        let (ref ak, ref av) = after.entries[ai];
        match bk.cmp(ak) {
            std::cmp::Ordering::Less => {
                diffs.push(SettingsDiffEntry {
                    id: bk.clone(),
                    kind: DiffKind::Removed,
                });
                bi += 1;
            }
            std::cmp::Ordering::Greater => {
                diffs.push(SettingsDiffEntry {
                    id: ak.clone(),
                    kind: DiffKind::Added,
                });
                ai += 1;
            }
            std::cmp::Ordering::Equal => {
                if bv != av {
                    diffs.push(SettingsDiffEntry {
                        id: bk.clone(),
                        kind: DiffKind::Changed {
                            old: bv.clone(),
                            new: av.clone(),
                        },
                    });
                }
                bi += 1;
                ai += 1;
            }
        }
    }
    while bi < before.entries.len() {
        diffs.push(SettingsDiffEntry {
            id: before.entries[bi].0.clone(),
            kind: DiffKind::Removed,
        });
        bi += 1;
    }
    while ai < after.entries.len() {
        diffs.push(SettingsDiffEntry {
            id: after.entries[ai].0.clone(),
            kind: DiffKind::Added,
        });
        ai += 1;
    }
    diffs
}

// ---------------------------------------------------------------------------
// SettingsHistory – track changes over time
// ---------------------------------------------------------------------------

/// Records value changes so they can be undone/redone.
#[derive(Debug, Clone)]
pub struct SettingsHistory {
    undo_stack: Vec<HistoryRecord>,
    redo_stack: Vec<HistoryRecord>,
}

/// A single change record.
#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub setting_id: String,
    pub old_value: String,
    pub new_value: String,
}

impl SettingsHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Record a change. Clears the redo stack.
    pub fn record(&mut self, setting_id: impl Into<String>, old_value: impl Into<String>, new_value: impl Into<String>) {
        self.undo_stack.push(HistoryRecord {
            setting_id: setting_id.into(),
            old_value: old_value.into(),
            new_value: new_value.into(),
        });
        self.redo_stack.clear();
    }

    /// Pop the most recent change and push it onto the redo stack.
    /// Returns the record so the caller can apply the old value.
    pub fn undo(&mut self) -> Option<HistoryRecord> {
        if let Some(rec) = self.undo_stack.pop() {
            self.redo_stack.push(rec.clone());
            Some(rec)
        } else {
            None
        }
    }

    /// Re-apply the most recently undone change.
    pub fn redo(&mut self) -> Option<HistoryRecord> {
        if let Some(rec) = self.redo_stack.pop() {
            self.undo_stack.push(rec.clone());
            Some(rec)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }
}

impl Default for SettingsHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

/// Reset all entries to their default values, returning how many were changed.
pub fn bulk_reset_to_defaults(entries: &mut [SettingEntry]) -> usize {
    let mut count = 0;
    for entry in entries.iter_mut() {
        if entry.current_value != entry.default_value {
            entry.current_value = entry.default_value.clone();
            entry.modified = false;
            count += 1;
        }
    }
    count
}

/// Apply a batch of `(setting_id, new_value)` pairs, returning how many matched.
pub fn bulk_apply(entries: &mut [SettingEntry], changes: &[(&str, &str)]) -> usize {
    let mut applied = 0;
    for (id, val) in changes {
        if let Some(entry) = entries.iter_mut().find(|e| e.id == *id) {
            entry.current_value = (*val).to_string();
            entry.modified = entry.current_value != entry.default_value;
            applied += 1;
        }
    }
    applied
}

// ---------------------------------------------------------------------------
// SettingsExporter – serialise to a simple key=value format
// ---------------------------------------------------------------------------

/// Export settings to a simple `key = value` text format (one per line).
pub fn export_as_kv(entries: &[SettingEntry], modified_only: bool) -> String {
    let mut out = String::new();
    for e in entries {
        if modified_only && !e.modified && e.current_value == e.default_value {
            continue;
        }
        out.push_str(&e.id);
        out.push_str(" = ");
        out.push_str(&e.current_value);
        out.push('\n');
    }
    out
}

/// Import settings from `key = value` lines, returning `(id, value)` pairs.
pub fn parse_kv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.splitn(2, '=');
            let key = parts.next()?.trim().to_string();
            let val = parts.next()?.trim().to_string();
            if key.is_empty() {
                return None;
            }
            Some((key, val))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SettingsAccessibility helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl fmt::Display for SettingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean => write!(f, "boolean"),
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Enum(opts) => write!(f, "enum({})", opts.join(", ")),
            Self::Array => write!(f, "array"),
            Self::Object => write!(f, "object"),
        }
    }
}

impl fmt::Display for SettingsScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "User"),
            Self::Workspace => write!(f, "Workspace"),
            Self::Folder => write!(f, "Folder"),
        }
    }
}

impl fmt::Display for SettingEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mod_marker = if self.modified { " [modified]" } else { "" };
        write!(
            f,
            "{} ({}) = {}{}",
            self.id, self.setting_type, self.current_value, mod_marker
        )
    }
}

// ---------------------------------------------------------------------------
// SettingEntry — additional methods
// ---------------------------------------------------------------------------

impl SettingEntry {
    /// Returns `true` if the current value differs from the default.
    pub fn is_modified(&self) -> bool {
        self.modified || self.current_value != self.default_value
    }

    /// Reset this entry to its default value and clear the modified flag.
    pub fn reset(&mut self) {
        self.current_value = self.default_value.clone();
        self.modified = false;
    }

    /// Validate that the current value is acceptable for the setting type.
    /// Returns an error message on failure, or `None` if valid.
    pub fn validate(&self) -> Option<String> {
        match &self.setting_type {
            SettingType::Boolean => {
                if self.current_value != "true" && self.current_value != "false" {
                    Some(format!(
                        "{}: expected 'true' or 'false', got '{}'",
                        self.id, self.current_value
                    ))
                } else {
                    None
                }
            }
            SettingType::Number => {
                if self.current_value.parse::<f64>().is_err() {
                    Some(format!(
                        "{}: expected a number, got '{}'",
                        self.id, self.current_value
                    ))
                } else {
                    None
                }
            }
            SettingType::Enum(opts) => {
                if !opts.contains(&self.current_value) {
                    Some(format!(
                        "{}: '{}' is not one of [{}]",
                        self.id,
                        self.current_value,
                        opts.join(", ")
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Return the category hierarchy as segments split on `.` in the id.
    /// For `editor.font.size` this returns `["editor", "font", "size"]`.
    pub fn id_segments(&self) -> Vec<&str> {
        self.id.split('.').collect()
    }

    /// The top-level namespace of the setting id (the part before the first dot).
    pub fn namespace(&self) -> &str {
        self.id.split('.').next().unwrap_or(&self.id)
    }
}

// ---------------------------------------------------------------------------
// SettingsView — additional methods
// ---------------------------------------------------------------------------

impl SettingsView {
    /// Return the currently selected `SettingEntry`, if any.
    pub fn selected_entry(&self) -> Option<&SettingEntry> {
        self.filtered_entries
            .get(self.selected_index)
            .and_then(|&idx| self.entries.get(idx))
    }

    /// Return a mutable reference to the currently selected entry.
    pub fn selected_entry_mut(&mut self) -> Option<&mut SettingEntry> {
        let idx = self.filtered_entries.get(self.selected_index).copied();
        idx.and_then(move |i| self.entries.get_mut(i))
    }

    /// Return the number of entries that have been modified.
    pub fn modified_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_modified()).count()
    }

    /// Validate all entries, returning a vec of error messages.
    pub fn validate_all(&self) -> Vec<String> {
        self.entries.iter().filter_map(|e| e.validate()).collect()
    }

    /// Set the active scope.
    pub fn set_scope(&mut self, scope: SettingsScope) {
        self.active_scope = scope;
    }

    /// Cycle to the next scope (User → Workspace → Folder → User).
    pub fn cycle_scope(&mut self) {
        self.active_scope = match self.active_scope {
            SettingsScope::User => SettingsScope::Workspace,
            SettingsScope::Workspace => SettingsScope::Folder,
            SettingsScope::Folder => SettingsScope::User,
        };
    }

    /// Return how many entries match the current filter.
    pub fn visible_count(&self) -> usize {
        self.filtered_entries.len()
    }

    /// Return an iterator over the entries visible after filtering.
    pub fn visible_entries(&self) -> impl Iterator<Item = &SettingEntry> {
        self.filtered_entries
            .iter()
            .filter_map(move |&idx| self.entries.get(idx))
    }

    /// Cycle the selected enum setting to its next variant.
    /// Returns `true` if a change was made.
    pub fn cycle_enum(&mut self, filtered_index: usize) -> bool {
        if let Some(&entry_idx) = self.filtered_entries.get(filtered_index) {
            if let Some(entry) = self.entries.get_mut(entry_idx) {
                if let SettingType::Enum(ref opts) = entry.setting_type {
                    if opts.is_empty() {
                        return false;
                    }
                    let current_pos = opts.iter().position(|o| *o == entry.current_value);
                    let next = match current_pos {
                        Some(pos) => (pos + 1) % opts.len(),
                        None => 0,
                    };
                    entry.current_value = opts[next].clone();
                    entry.modified = entry.current_value != entry.default_value;
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// SettingsTreeNode — additional methods
// ---------------------------------------------------------------------------

impl SettingsTreeNode {
    /// Count the total number of leaf nodes (settings) in this subtree.
    pub fn leaf_count(&self) -> usize {
        if self.is_leaf() {
            return 1;
        }
        self.children.iter().map(|c| c.leaf_count()).sum()
    }

    /// Expand all nodes in the subtree recursively.
    pub fn expand_all(&mut self) {
        self.expanded = true;
        for child in &mut self.children {
            child.expand_all();
        }
    }

    /// Collapse all nodes in the subtree recursively.
    pub fn collapse_all(&mut self) {
        self.expanded = false;
        for child in &mut self.children {
            child.collapse_all();
        }
    }

    /// Collect all entry indices from leaf nodes in depth-first order.
    pub fn collect_entry_indices(&self) -> Vec<usize> {
        let mut out = Vec::new();
        self.collect_entry_indices_inner(&mut out);
        out
    }

    fn collect_entry_indices_inner(&self, out: &mut Vec<usize>) {
        if let Some(idx) = self.entry_index {
            out.push(idx);
        }
        for child in &self.children {
            child.collect_entry_indices_inner(out);
        }
    }

    /// Return the maximum depth of the subtree.
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            return 0;
        }
        1 + self.children.iter().map(|c| c.max_depth()).max().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// SettingsSnapshot — additional methods
// ---------------------------------------------------------------------------

impl SettingsSnapshot {
    /// Return an iterator over all `(id, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Return the ids of settings whose values differ between two snapshots.
    pub fn changed_ids(&self, other: &SettingsSnapshot) -> Vec<String> {
        diff_snapshots(self, other)
            .into_iter()
            .map(|d| d.id)
            .collect()
    }
}

/// Summary line for screen readers: "{title}: {value} ({type})".
pub fn accessibility_label(entry: &SettingEntry) -> String {
    let type_label = match &entry.setting_type {
        SettingType::Boolean => "toggle",
        SettingType::String => "text",
        SettingType::Number => "number",
        SettingType::Enum(_) => "dropdown",
        SettingType::Array => "list",
        SettingType::Object => "object",
    };
    let modified = if entry.modified { ", modified" } else { "" };
    format!(
        "{}: {} ({}{})",
        entry.title, entry.current_value, type_label, modified
    )
}


// ---------------------------------------------------------------------------
// SettingsCategoryTree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SettingsCategoryTree {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl SettingsCategoryTree {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for SettingsCategoryTree {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for SettingsCategoryTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SettingsCategoryTree({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// SettingsModifiedCounter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SettingsModifiedCounter {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl SettingsModifiedCounter {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for SettingsModifiedCounter {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for SettingsModifiedCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SettingsModifiedCounter({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// SettingsCategoryTreeSnapshot — point-in-time snapshot of SettingsCategoryTree state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SettingsCategoryTreeSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl SettingsCategoryTreeSnapshot {
    pub fn capture(source: &SettingsCategoryTree, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for SettingsCategoryTreeSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// SettingsModifiedCounterStats — aggregate statistics for SettingsModifiedCounter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SettingsModifiedCounterStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl SettingsModifiedCounterStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for SettingsModifiedCounterStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// SettingsCategoryTreeConfig — configuration for SettingsCategoryTree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SettingsCategoryTreeConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl SettingsCategoryTreeConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for SettingsCategoryTreeConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for SettingsCategoryTreeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// SettingsSearchIndex – index settings for fast search
// ---------------------------------------------------------------------------

/// Index of settings for fast keyword search.
#[derive(Debug, Clone, Default)]
pub struct SettingsSearchIndexV2 {
    entries: Vec<(String, String, String, String)>, // (key, title, description, category)
}

impl SettingsSearchIndexV2 {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add a setting to the search index.
    pub fn add_setting(&mut self, key: &str, title: &str, description: &str, category: &str) {
        self.entries.push((
            key.to_lowercase(),
            title.to_lowercase(),
            description.to_lowercase(),
            category.to_string(),
        ));
    }

    /// Search for settings matching the query (case-insensitive substring).
    pub fn search(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, (key, title, desc, _))| {
                key.contains(&q) || title.contains(&q) || desc.contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Return results ranked by relevance (key match > title match > desc match).
    pub fn ranked_results(&self, query: &str) -> Vec<(usize, u32)> {
        let q = query.to_lowercase();
        let mut results: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, (key, title, desc, _))| {
                let score = if key.contains(&q) {
                    3
                } else if title.contains(&q) {
                    2
                } else if desc.contains(&q) {
                    1
                } else {
                    return None;
                };
                Some((i, score))
            })
            .collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }

    /// Suggest a correction for a typo using edit distance.
    pub fn suggestion_for_typo(&self, query: &str) -> Option<String> {
        let q = query.to_lowercase();
        let mut best: Option<(String, usize)> = None;
        for (key, title, _, _) in &self.entries {
            for candidate in [key, title] {
                let dist = edit_distance(&q, candidate);
                if dist <= 3 {
                    if best.as_ref().map_or(true, |(_, d)| dist < *d) {
                        best = Some((candidate.clone(), dist));
                    }
                }
            }
        }
        best.map(|(s, _)| s)
    }
}

/// Simple edit distance (Levenshtein).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() { dp[i][0] = i; }
    for j in 0..=b.len() { dp[0][j] = j; }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

// ---------------------------------------------------------------------------
// SettingsDiffCalculator – diff two configs
// ---------------------------------------------------------------------------

/// Compute the difference between two configuration maps.
#[derive(Debug, Clone, Default)]
pub struct SettingsDiffCalculator {
    added: Vec<(String, String)>,
    removed: Vec<String>,
    changed: Vec<(String, String, String)>, // (key, old, new)
}

impl SettingsDiffCalculator {
    /// Compute a diff between `old` and `new` config maps.
    pub fn compute(old: &HashMap<String, String>, new: &HashMap<String, String>) -> Self {
        let mut diff = Self::default();
        for (k, v) in new {
            match old.get(k) {
                None => diff.added.push((k.clone(), v.clone())),
                Some(old_v) if old_v != v => diff.changed.push((k.clone(), old_v.clone(), v.clone())),
                _ => {}
            }
        }
        for k in old.keys() {
            if !new.contains_key(k) {
                diff.removed.push(k.clone());
            }
        }
        diff
    }

    pub fn added_entries(&self) -> &[(String, String)] { &self.added }
    pub fn removed_keys(&self) -> &[String] { &self.removed }
    pub fn changed_entries(&self) -> &[(String, String, String)] { &self.changed }

    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }

    pub fn is_modified(&self) -> bool { self.change_count() > 0 }

    pub fn changed_keys(&self) -> Vec<&str> {
        self.changed.iter().map(|(k, _, _)| k.as_str()).collect()
    }

    /// Apply this diff to a base config, producing the new config.
    pub fn apply_diff(&self, base: &HashMap<String, String>) -> HashMap<String, String> {
        let mut result = base.clone();
        for k in &self.removed {
            result.remove(k);
        }
        for (k, v) in &self.added {
            result.insert(k.clone(), v.clone());
        }
        for (k, _, new_v) in &self.changed {
            result.insert(k.clone(), new_v.clone());
        }
        result
    }
}

// ---------------------------------------------------------------------------
// SettingsBreadcrumb – navigation breadcrumb
// ---------------------------------------------------------------------------

/// A breadcrumb trail for navigating settings categories.
#[derive(Debug, Clone, Default)]
pub struct SettingsBreadcrumbV2 {
    segments: Vec<String>,
}

impl SettingsBreadcrumbV2 {
    pub fn new() -> Self { Self { segments: Vec::new() } }

    pub fn push(&mut self, segment: &str) {
        self.segments.push(segment.to_string());
    }

    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    pub fn current(&self) -> Option<&str> {
        self.segments.last().map(|s| s.as_str())
    }

    pub fn path_string(&self) -> String {
        self.segments.join(" > ")
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Navigate to a specific depth, truncating deeper segments.
    pub fn navigate_to_depth(&mut self, depth: usize) {
        self.segments.truncate(depth);
    }

    pub fn root(&self) -> Option<&str> {
        self.segments.first().map(|s| s.as_str())
    }
}


/// Configuration manager for settings_view functionality.
pub struct SettingsViewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SettingsViewConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &SettingsViewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for settings_view operations.
pub struct SettingsViewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SettingsViewRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for settings_view.
pub struct SettingsViewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SettingsViewValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &SettingsViewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for settings_view
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSettingsViewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSettingsViewRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaSettingsViewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSettingsViewCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaSettingsViewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 158
// ---------------------------------------------------------------------------

/// Generic object pool `Xc158Pool<T>`.
pub struct Xc158Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc158Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc158PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc158Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc158PoolStats {
        Xc158PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc158Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc158Scheduler`.
pub struct Xc158Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc158Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc158Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_158 hash for the given byte slice.
pub fn xc_158_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_158 convention.
pub fn xc_158_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_60 deepening: state machine + event bus ---

/// States for the Xd60 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd60State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd60State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd60Transition {
    pub from: Xd60State,
    pub to: Xd60State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd60StateMachine {
    current: Xd60State,
    history: Vec<Xd60Transition>,
    step_counter: usize,
}

impl Xd60StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd60State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd60State {
        self.current
    }

    pub fn history(&self) -> &[Xd60Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd60State) -> Result<Xd60State, String> {
        let allowed = match (self.current, target) {
            (Xd60State::Idle, Xd60State::Running) => true,
            (Xd60State::Running, Xd60State::Paused) => true,
            (Xd60State::Running, Xd60State::Done) => true,
            (Xd60State::Paused, Xd60State::Running) => true,
            (Xd60State::Paused, Xd60State::Done) => true,
            (Xd60State::Done, Xd60State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_60: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd60Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd60SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd60State> {
        let prefix = "Xd60SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd60State::Idle),
            "Running" => Some(Xd60State::Running),
            "Paused" => Some(Xd60State::Paused),
            "Done" => Some(Xd60State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd60State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd60 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd60Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd60Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd60HandlerFn = Box<dyn Fn(&Xd60Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd60EventBus {
    handlers: Vec<(usize, Option<String>, Xd60HandlerFn)>,
    next_id: usize,
    published: Vec<Xd60Event>,
}

impl Xd60EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd60Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd60Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd60Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd60Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #58
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf58Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf58TrieNode {
    children: std::collections::HashMap<char, Xf58TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf58Trie {
    root: Xf58TrieNode,
    count: usize,
}

impl Xf58Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf58TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf58TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf58TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf58BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf58BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 157).
pub struct Xh157SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh157SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 199 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 157).
pub struct Xh157BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh157BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 157).
pub struct Xi157Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi157Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi157Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi157Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 157).
pub struct Xi157IntervalTree {
    xi_intervals: Vec<Xi157Interval>,
}

impl Xi157IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi157Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi157Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi157Interval) -> Vec<&Xi157Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi157Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi157Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi157Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi157Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi157Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi157Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 157) ---

/// Disjoint set / union-find for crate 157.
pub struct Xj157UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj157UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ157_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 157.
pub struct Xj157BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj157BTreeNode<K, V>>>,
    len: usize,
}

struct Xj157BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj157BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj157BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ157_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ157_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj157BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj157BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj157BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj157BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_157 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk157SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk157SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk157DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk157DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
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

    #[test]
    fn breadcrumb_empty() {
        let bc = SettingsBreadcrumb::new();
        assert!(bc.is_empty());
        assert_eq!(bc.depth(), 0);
        assert_eq!(bc.display(), "");
    }

    #[test]
    fn breadcrumb_push_display() {
        let mut bc = SettingsBreadcrumb::new();
        bc.push("User");
        bc.push("Editor");
        bc.push("Font");
        assert_eq!(bc.display(), "User > Editor > Font");
        assert_eq!(bc.depth(), 3);
    }

    #[test]
    fn breadcrumb_pop() {
        let mut bc = SettingsBreadcrumb::new();
        bc.push("A");
        bc.push("B");
        assert_eq!(bc.pop(), Some("B".to_string()));
        assert_eq!(bc.display(), "A");
    }

    #[test]
    fn breadcrumb_pop_empty() {
        let mut bc = SettingsBreadcrumb::new();
        assert_eq!(bc.pop(), None);
    }

    #[test]
    fn breadcrumb_single_segment() {
        let mut bc = SettingsBreadcrumb::new();
        bc.push("Root");
        assert_eq!(bc.display(), "Root");
        assert!(!bc.is_empty());
    }

    #[test]
    fn breadcrumb_default() {
        let bc = SettingsBreadcrumb::default();
        assert!(bc.is_empty());
    }

    // -----------------------------------------------------------------------
    // SettingsTreeNode tests
    // -----------------------------------------------------------------------

    #[test]
    fn tree_node_basic() {
        let mut node = SettingsTreeNode::new("root", "Root");
        assert!(node.is_leaf());
        assert_eq!(node.child_count(), 0);
        node.add_child(SettingsTreeNode::new("a", "A"));
        assert!(!node.is_leaf());
        assert_eq!(node.child_count(), 1);
    }

    #[test]
    fn tree_node_toggle_expand() {
        let mut node = SettingsTreeNode::new("n", "N");
        assert!(!node.expanded);
        node.toggle_expand();
        assert!(node.expanded);
        node.toggle_expand();
        assert!(!node.expanded);
    }

    #[test]
    fn tree_node_find_by_key() {
        let mut root = SettingsTreeNode::new("root", "Root");
        let mut child = SettingsTreeNode::new("editor", "Editor");
        child.add_child(SettingsTreeNode::new("fontSize", "Font Size"));
        root.add_child(child);

        assert!(root.find_by_key("fontSize").is_some());
        assert!(root.find_by_key("editor").is_some());
        assert!(root.find_by_key("missing").is_none());
    }

    #[test]
    fn build_tree_from_entries() {
        let entries = sample_entries();
        let tree = build_tree(&entries);

        assert_eq!(tree.key, "root");
        assert!(tree.expanded);
        // Should have two top-level groups: "editor" and "terminal"
        assert_eq!(tree.child_count(), 2);

        let editor = tree.find_by_key("editor").unwrap();
        // editor has fontSize, wordWrap, minimap
        assert_eq!(editor.child_count(), 3);
        assert!(!editor.is_leaf());

        let font_size = tree.find_by_key("fontSize").unwrap();
        assert!(font_size.is_leaf());
        assert_eq!(font_size.entry_index, Some(0));

        // minimap is an intermediate node with child "enabled"
        let minimap = tree.find_by_key("minimap").unwrap();
        assert!(!minimap.is_leaf());
        let enabled = tree.find_by_key("enabled").unwrap();
        assert!(enabled.is_leaf());
        assert_eq!(enabled.entry_index, Some(2));
    }

    // -----------------------------------------------------------------------
    // SettingsSearchIndex tests
    // -----------------------------------------------------------------------

    #[test]
    fn search_index_basic() {
        let entries = sample_entries();
        let index = SettingsSearchIndex::build(&entries);
        assert_eq!(index.entry_count(), 4);

        // "font" matches fontSize (0) and terminal fontFamily (3)
        let results = index.search("font");
        assert!(results.contains(&0));
        assert!(results.contains(&3));

        // empty query returns all
        assert_eq!(index.search("").len(), 4);
    }

    #[test]
    fn search_index_case_insensitive() {
        let entries = sample_entries();
        let index = SettingsSearchIndex::build(&entries);

        let r1 = index.search("MINIMAP");
        let r2 = index.search("minimap");
        assert_eq!(r1, r2);
        assert!(r1.contains(&2));
    }

    #[test]
    fn search_index_multi_word() {
        let entries = sample_entries();
        let index = SettingsSearchIndex::build(&entries);

        // "font terminal" should match only entry 3 (terminal.fontFamily)
        let results = index.search("font terminal");
        assert_eq!(results, vec![3]);
    }

    // -----------------------------------------------------------------------
    // filter_modified tests
    // -----------------------------------------------------------------------

    #[test]
    fn filter_modified_returns_changed() {
        let mut entries = sample_entries();
        // nothing modified yet
        assert!(filter_modified(&entries).is_empty());

        // modify one entry via the flag
        entries[1].modified = true;
        let modified = filter_modified(&entries);
        assert_eq!(modified, vec![1]);

        // modify another by changing its value
        entries[3].current_value = "Courier".to_string();
        let modified = filter_modified(&entries);
        assert_eq!(modified, vec![1, 3]);
    }

    // -----------------------------------------------------------------------
    // SettingsSnapshot + SettingsDiff tests
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_capture_and_get() {
        let entries = sample_entries();
        let snap = SettingsSnapshot::capture(&entries, "v1");
        assert_eq!(snap.len(), 4);
        assert!(!snap.is_empty());
        assert_eq!(snap.label, "v1");
        assert_eq!(snap.get("editor.fontSize"), Some("14"));
        assert_eq!(snap.get("terminal.fontFamily"), Some("monospace"));
        assert_eq!(snap.get("nonexistent"), None);
    }

    #[test]
    fn diff_snapshots_detects_changes() {
        let mut entries = sample_entries();
        let before = SettingsSnapshot::capture(&entries, "before");

        entries[0].current_value = "18".to_string();
        let after = SettingsSnapshot::capture(&entries, "after");

        let diffs = diff_snapshots(&before, &after);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].id, "editor.fontSize");
        assert_eq!(
            diffs[0].kind,
            DiffKind::Changed {
                old: "14".to_string(),
                new: "18".to_string(),
            }
        );
    }

    #[test]
    fn diff_snapshots_added_removed() {
        let entries_a = vec![sample_entries()[0].clone()];
        let entries_b = vec![sample_entries()[3].clone()];
        let snap_a = SettingsSnapshot::capture(&entries_a, "a");
        let snap_b = SettingsSnapshot::capture(&entries_b, "b");

        let diffs = diff_snapshots(&snap_a, &snap_b);
        assert_eq!(diffs.len(), 2);
        // editor.fontSize removed, terminal.fontFamily added
        assert!(diffs.iter().any(|d| d.id == "editor.fontSize" && d.kind == DiffKind::Removed));
        assert!(diffs.iter().any(|d| d.id == "terminal.fontFamily" && d.kind == DiffKind::Added));
    }

    // -----------------------------------------------------------------------
    // SettingsHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn history_undo_redo() {
        let mut h = SettingsHistory::new();
        assert!(!h.can_undo());
        assert!(!h.can_redo());

        h.record("editor.fontSize", "14", "18");
        h.record("editor.wordWrap", "off", "on");
        assert_eq!(h.undo_count(), 2);

        let rec = h.undo().unwrap();
        assert_eq!(rec.setting_id, "editor.wordWrap");
        assert_eq!(rec.old_value, "off");
        assert!(h.can_redo());

        let rec = h.redo().unwrap();
        assert_eq!(rec.setting_id, "editor.wordWrap");
        assert_eq!(rec.new_value, "on");
        assert!(!h.can_redo());
    }

    #[test]
    fn history_record_clears_redo() {
        let mut h = SettingsHistory::new();
        h.record("a", "1", "2");
        h.undo();
        assert!(h.can_redo());
        h.record("b", "3", "4");
        assert!(!h.can_redo());
    }

    // -----------------------------------------------------------------------
    // Bulk operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn bulk_reset_to_defaults_resets_modified() {
        let mut entries = sample_entries();
        entries[0].current_value = "20".to_string();
        entries[0].modified = true;
        entries[2].current_value = "false".to_string();
        entries[2].modified = true;

        let count = bulk_reset_to_defaults(&mut entries);
        assert_eq!(count, 2);
        assert_eq!(entries[0].current_value, "14");
        assert!(!entries[0].modified);
        assert_eq!(entries[2].current_value, "true");
    }

    #[test]
    fn bulk_apply_applies_matching() {
        let mut entries = sample_entries();
        let changes = vec![
            ("editor.fontSize", "20"),
            ("terminal.fontFamily", "Consolas"),
            ("nonexistent.key", "ignored"),
        ];
        let applied = bulk_apply(&mut entries, &changes);
        assert_eq!(applied, 2);
        assert_eq!(entries[0].current_value, "20");
        assert!(entries[0].modified);
        assert_eq!(entries[3].current_value, "Consolas");
    }

    // -----------------------------------------------------------------------
    // Export/import tests
    // -----------------------------------------------------------------------

    #[test]
    fn export_kv_all() {
        let entries = sample_entries();
        let text = export_as_kv(&entries, false);
        assert!(text.contains("editor.fontSize = 14\n"));
        assert!(text.contains("terminal.fontFamily = monospace\n"));
    }

    #[test]
    fn export_kv_modified_only() {
        let mut entries = sample_entries();
        let text = export_as_kv(&entries, true);
        assert!(text.is_empty());

        entries[0].current_value = "20".to_string();
        entries[0].modified = true;
        let text = export_as_kv(&entries, true);
        assert_eq!(text, "editor.fontSize = 20\n");
    }

    #[test]
    fn parse_kv_roundtrip() {
        let input = "editor.fontSize = 20\n# comment\n\nterminal.fontFamily = Consolas\n";
        let pairs = parse_kv(input);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("editor.fontSize".to_string(), "20".to_string()));
        assert_eq!(pairs[1], ("terminal.fontFamily".to_string(), "Consolas".to_string()));
    }

    // -----------------------------------------------------------------------
    // Accessibility tests
    // -----------------------------------------------------------------------

    #[test]
    fn accessibility_label_format() {
        let entry = &sample_entries()[0];
        let label = accessibility_label(entry);
        assert_eq!(label, "Font Size: 14 (number)");
    }

    #[test]
    fn accessibility_label_modified() {
        let mut entry = sample_entries()[0].clone();
        entry.modified = true;
        let label = accessibility_label(&entry);
        assert!(label.contains(", modified"));
    }

    // -----------------------------------------------------------------------
    // Display impl tests
    // -----------------------------------------------------------------------

    #[test]
    fn setting_type_display() {
        assert_eq!(SettingType::Boolean.to_string(), "boolean");
        assert_eq!(SettingType::String.to_string(), "string");
        assert_eq!(SettingType::Number.to_string(), "number");
        assert_eq!(SettingType::Array.to_string(), "array");
        assert_eq!(SettingType::Object.to_string(), "object");
        let e = SettingType::Enum(vec!["a".into(), "b".into()]);
        assert_eq!(e.to_string(), "enum(a, b)");
    }

    #[test]
    fn settings_scope_display() {
        assert_eq!(SettingsScope::User.to_string(), "User");
        assert_eq!(SettingsScope::Workspace.to_string(), "Workspace");
        assert_eq!(SettingsScope::Folder.to_string(), "Folder");
    }

    #[test]
    fn setting_entry_display() {
        let entry = &sample_entries()[0];
        let s = entry.to_string();
        assert!(s.contains("editor.fontSize"));
        assert!(s.contains("14"));
        assert!(!s.contains("[modified]"));

        let mut modified = sample_entries()[0].clone();
        modified.modified = true;
        assert!(modified.to_string().contains("[modified]"));
    }

    // -----------------------------------------------------------------------
    // SettingEntry additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn entry_is_modified() {
        let mut entry = sample_entries()[0].clone();
        assert!(!entry.is_modified());
        entry.current_value = "99".to_string();
        assert!(entry.is_modified());
    }

    #[test]
    fn entry_reset() {
        let mut entry = sample_entries()[0].clone();
        entry.current_value = "99".to_string();
        entry.modified = true;
        entry.reset();
        assert_eq!(entry.current_value, "14");
        assert!(!entry.modified);
    }

    #[test]
    fn entry_validate_boolean_ok() {
        let entry = sample_entries()[2].clone(); // Boolean, value "true"
        assert!(entry.validate().is_none());
    }

    #[test]
    fn entry_validate_boolean_bad() {
        let mut entry = sample_entries()[2].clone();
        entry.current_value = "maybe".to_string();
        let err = entry.validate().unwrap();
        assert!(err.contains("expected 'true' or 'false'"));
    }

    #[test]
    fn entry_validate_number_ok() {
        let entry = sample_entries()[0].clone(); // Number, value "14"
        assert!(entry.validate().is_none());
    }

    #[test]
    fn entry_validate_number_bad() {
        let mut entry = sample_entries()[0].clone();
        entry.current_value = "abc".to_string();
        let err = entry.validate().unwrap();
        assert!(err.contains("expected a number"));
    }

    #[test]
    fn entry_validate_enum_ok() {
        let entry = sample_entries()[1].clone(); // Enum, value "off"
        assert!(entry.validate().is_none());
    }

    #[test]
    fn entry_validate_enum_bad() {
        let mut entry = sample_entries()[1].clone();
        entry.current_value = "invalid".to_string();
        let err = entry.validate().unwrap();
        assert!(err.contains("is not one of"));
    }

    #[test]
    fn entry_validate_string_always_ok() {
        let entry = sample_entries()[3].clone(); // String type
        assert!(entry.validate().is_none());
    }

    #[test]
    fn entry_id_segments() {
        let entry = &sample_entries()[2]; // editor.minimap.enabled
        let segs = entry.id_segments();
        assert_eq!(segs, vec!["editor", "minimap", "enabled"]);
    }

    #[test]
    fn entry_namespace() {
        assert_eq!(sample_entries()[0].namespace(), "editor");
        assert_eq!(sample_entries()[3].namespace(), "terminal");
    }

    // -----------------------------------------------------------------------
    // SettingsView additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn view_selected_entry() {
        let mut v = SettingsView::new();
        assert!(v.selected_entry().is_none());
        for e in sample_entries() {
            v.add_entry(e);
        }
        let sel = v.selected_entry().unwrap();
        assert_eq!(sel.id, "editor.fontSize");
        v.select_next();
        let sel = v.selected_entry().unwrap();
        assert_eq!(sel.id, "editor.wordWrap");
    }

    #[test]
    fn view_selected_entry_mut() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        if let Some(entry) = v.selected_entry_mut() {
            entry.current_value = "42".to_string();
            entry.modified = true;
        }
        assert_eq!(v.entries[0].current_value, "42");
    }

    #[test]
    fn view_modified_count() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        assert_eq!(v.modified_count(), 0);
        v.update_value(0, "20");
        assert_eq!(v.modified_count(), 1);
        v.update_value(1, "on");
        assert_eq!(v.modified_count(), 2);
    }

    #[test]
    fn view_validate_all() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        assert!(v.validate_all().is_empty());
        v.entries[2].current_value = "maybe".to_string(); // bad bool
        let errs = v.validate_all();
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn view_cycle_scope() {
        let mut v = SettingsView::new();
        assert_eq!(v.active_scope, SettingsScope::User);
        v.cycle_scope();
        assert_eq!(v.active_scope, SettingsScope::Workspace);
        v.cycle_scope();
        assert_eq!(v.active_scope, SettingsScope::Folder);
        v.cycle_scope();
        assert_eq!(v.active_scope, SettingsScope::User);
    }

    #[test]
    fn view_set_scope() {
        let mut v = SettingsView::new();
        v.set_scope(SettingsScope::Folder);
        assert_eq!(v.active_scope, SettingsScope::Folder);
    }

    #[test]
    fn view_visible_count() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        assert_eq!(v.visible_count(), 4);
        v.filter_by_query("font");
        assert_eq!(v.visible_count(), 2);
    }

    #[test]
    fn view_visible_entries_iter() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        v.filter_by_query("terminal");
        let ids: Vec<&str> = v.visible_entries().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["terminal.fontFamily"]);
    }

    #[test]
    fn view_cycle_enum() {
        let mut v = SettingsView::new();
        for e in sample_entries() {
            v.add_entry(e);
        }
        // wordWrap is Enum with ["off", "on", "bounded"], default "off"
        let pos = v
            .filtered_entries
            .iter()
            .position(|&i| v.entries[i].id == "editor.wordWrap")
            .unwrap();
        assert!(v.cycle_enum(pos));
        assert_eq!(v.entries[1].current_value, "on");
        assert!(v.cycle_enum(pos));
        assert_eq!(v.entries[1].current_value, "bounded");
        assert!(v.cycle_enum(pos));
        assert_eq!(v.entries[1].current_value, "off"); // wraps around

        // cycling a non-enum entry returns false
        let bool_pos = v
            .filtered_entries
            .iter()
            .position(|&i| v.entries[i].id == "editor.minimap.enabled")
            .unwrap();
        assert!(!v.cycle_enum(bool_pos));
    }

    // -----------------------------------------------------------------------
    // SettingsTreeNode additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn tree_leaf_count() {
        let entries = sample_entries();
        let tree = build_tree(&entries);
        assert_eq!(tree.leaf_count(), 4);

        let editor = tree.find_by_key("editor").unwrap();
        assert_eq!(editor.leaf_count(), 3);
    }

    #[test]
    fn tree_expand_collapse_all() {
        let entries = sample_entries();
        let mut tree = build_tree(&entries);
        tree.expand_all();
        // All nodes should be expanded
        assert!(tree.find_by_key("editor").unwrap().expanded);
        assert!(tree.find_by_key("minimap").unwrap().expanded);

        tree.collapse_all();
        assert!(!tree.find_by_key("editor").unwrap().expanded);
        assert!(!tree.expanded);
    }

    #[test]
    fn tree_collect_entry_indices() {
        let entries = sample_entries();
        let tree = build_tree(&entries);
        let indices = tree.collect_entry_indices();
        assert_eq!(indices.len(), 4);
        // All entry indices should be present
        for i in 0..4 {
            assert!(indices.contains(&i), "missing index {}", i);
        }
    }

    #[test]
    fn tree_max_depth() {
        let entries = sample_entries();
        let tree = build_tree(&entries);
        // root -> editor -> minimap -> enabled is depth 3
        assert_eq!(tree.max_depth(), 3);

        let leaf = SettingsTreeNode::new("leaf", "Leaf");
        assert_eq!(leaf.max_depth(), 0);
    }

    // -----------------------------------------------------------------------
    // SettingsSnapshot additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_iter() {
        let entries = sample_entries();
        let snap = SettingsSnapshot::capture(&entries, "test");
        let pairs: Vec<(&str, &str)> = snap.iter().collect();
        assert_eq!(pairs.len(), 4);
        // Sorted by id
        assert_eq!(pairs[0].0, "editor.fontSize");
    }

    #[test]
    fn snapshot_changed_ids() {
        let entries = sample_entries();
        let snap1 = SettingsSnapshot::capture(&entries, "before");

        let mut entries2 = sample_entries();
        entries2[0].current_value = "20".to_string();
        entries2[3].current_value = "Courier".to_string();
        let snap2 = SettingsSnapshot::capture(&entries2, "after");

        let changed = snap1.changed_ids(&snap2);
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&"editor.fontSize".to_string()));
        assert!(changed.contains(&"terminal.fontFamily".to_string()));
    }

    #[test] fn settingsCategoryTree_new() { let s = SettingsCategoryTree::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn settingsCategoryTree_add() { let mut s = SettingsCategoryTree::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn settingsCategoryTree_remove() { let mut s = SettingsCategoryTree::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn settingsCategoryTree_config() { let mut s = SettingsCategoryTree::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn settingsCategoryTree_nav() { let mut s = SettingsCategoryTree::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn settingsCategoryTree_filter() { let mut s = SettingsCategoryTree::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn settingsCategoryTree_display() { assert!(format!("{}", SettingsCategoryTree::new()).contains("SettingsCategoryTree")); }
    #[test] fn settingsModifiedCounter_new() { let s = SettingsModifiedCounter::new(); assert!(s.is_empty()); }
    #[test] fn settingsModifiedCounter_add() { let mut s = SettingsModifiedCounter::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn settingsModifiedCounter_active() { let mut s = SettingsModifiedCounter::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn settingsModifiedCounter_error() { let mut s = SettingsModifiedCounter::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn settingsModifiedCounter_rm_group() { let mut s = SettingsModifiedCounter::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn settingsModifiedCounter_display() { assert!(format!("{}", SettingsModifiedCounter::new()).contains("SettingsModifiedCounter")); }


    #[test] fn settingsCategoryTree_snap_capture() {
        let s = SettingsCategoryTree::new();
        let snap = SettingsCategoryTreeSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn settingsCategoryTree_snap_stale() {
        let s = SettingsCategoryTree::new();
        let snap = SettingsCategoryTreeSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn settingsCategoryTree_snap_diff() {
        let s = SettingsCategoryTree::new();
        let s1v = SettingsCategoryTreeSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn settingsCategoryTree_snap_display() {
        let s = SettingsCategoryTree::new();
        let snap = SettingsCategoryTreeSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn settingsModifiedCounter_stats_record() {
        let mut st = SettingsModifiedCounterStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn settingsModifiedCounter_stats_hit_ratio() {
        let mut st = SettingsModifiedCounterStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn settingsModifiedCounter_stats_merge() {
        let mut a = SettingsModifiedCounterStats::new();
        a.total_adds = 5;
        let mut b = SettingsModifiedCounterStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn settingsModifiedCounter_stats_display() {
        let st = SettingsModifiedCounterStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn settingsCategoryTree_config_default() {
        let c = SettingsCategoryTreeConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn settingsCategoryTree_config_builder() {
        let c = SettingsCategoryTreeConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn settingsCategoryTree_config_labels() {
        let mut c = SettingsCategoryTreeConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn settingsCategoryTree_config_cleanup_threshold() {
        let c = SettingsCategoryTreeConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn settingsCategoryTree_config_display() {
        assert!(format!("{}", SettingsCategoryTreeConfig::new()).contains("Config"));
    }
    #[test] fn settingsModifiedCounter_stats_peaks() {
        let mut st = SettingsModifiedCounterStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- SettingsSearchIndexV2 ------------------------------------------------

    #[test]
    fn search_index_basic_v2() {
        let mut idx = SettingsSearchIndexV2::new();
        idx.add_setting("editor.fontSize", "Font Size", "Controls font size", "Editor");
        idx.add_setting("editor.tabSize", "Tab Size", "Number of spaces for tab", "Editor");
        let results = idx.search("font");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 0);
    }

    #[test]
    fn search_index_no_match() {
        let mut idx = SettingsSearchIndexV2::new();
        idx.add_setting("a", "b", "c", "d");
        assert!(idx.search("xyz").is_empty());
    }

    #[test]
    fn search_index_ranked() {
        let mut idx = SettingsSearchIndexV2::new();
        idx.add_setting("font", "Other", "desc", "Cat");
        idx.add_setting("other", "font title", "desc", "Cat");
        idx.add_setting("other", "Other", "font in desc", "Cat");
        let ranked = idx.ranked_results("font");
        assert_eq!(ranked.len(), 3);
        assert!(ranked[0].1 >= ranked[1].1);
    }

    #[test]
    fn search_index_typo_suggestion() {
        let mut idx = SettingsSearchIndexV2::new();
        idx.add_setting("editor.fontsize", "Font Size", "desc", "Editor");
        let suggestion = idx.suggestion_for_typo("fontsze");
        assert!(suggestion.is_some());
    }

    // -- SettingsDiffCalculator ---------------------------------------------

    #[test]
    fn diff_no_changes() {
        let a: HashMap<String, String> = [("k".into(), "v".into())].into_iter().collect();
        let diff = SettingsDiffCalculator::compute(&a, &a);
        assert!(!diff.is_modified());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn diff_added_removed_changed() {
        let old: HashMap<String, String> = [("a".into(), "1".into()), ("b".into(), "2".into())].into_iter().collect();
        let new: HashMap<String, String> = [("b".into(), "3".into()), ("c".into(), "4".into())].into_iter().collect();
        let diff = SettingsDiffCalculator::compute(&old, &new);
        assert_eq!(diff.added_entries().len(), 1);
        assert_eq!(diff.removed_keys().len(), 1);
        assert_eq!(diff.changed_keys(), vec!["b"]);
        assert!(diff.is_modified());
    }

    #[test]
    fn diff_apply() {
        let old: HashMap<String, String> = [("a".into(), "1".into())].into_iter().collect();
        let new: HashMap<String, String> = [("a".into(), "2".into()), ("b".into(), "3".into())].into_iter().collect();
        let diff = SettingsDiffCalculator::compute(&old, &new);
        let result = diff.apply_diff(&old);
        assert_eq!(result.get("a").unwrap(), "2");
        assert_eq!(result.get("b").unwrap(), "3");
    }

    // -- SettingsBreadcrumbV2 -------------------------------------------------

    #[test]
    fn breadcrumb_push_pop() {
        let mut bc = SettingsBreadcrumbV2::new();
        bc.push("Editor");
        bc.push("Font");
        assert_eq!(bc.current(), Some("Font"));
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.pop(), Some("Font".to_string()));
        assert_eq!(bc.current(), Some("Editor"));
    }

    #[test]
    fn breadcrumb_path_string() {
        let mut bc = SettingsBreadcrumbV2::new();
        bc.push("Settings");
        bc.push("Editor");
        bc.push("Font");
        assert_eq!(bc.path_string(), "Settings > Editor > Font");
    }

    #[test]
    fn breadcrumb_navigate_to_depth() {
        let mut bc = SettingsBreadcrumbV2::new();
        bc.push("A");
        bc.push("B");
        bc.push("C");
        bc.navigate_to_depth(1);
        assert_eq!(bc.depth(), 1);
        assert_eq!(bc.current(), Some("A"));
    }

    #[test]
    fn breadcrumb_root() {
        let mut bc = SettingsBreadcrumbV2::new();
        assert!(bc.root().is_none());
        bc.push("Root");
        bc.push("Child");
        assert_eq!(bc.root(), Some("Root"));
    }


    #[test]
    fn settings_view_config_new() {
        let cfg = SettingsViewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn settings_view_config_set_get() {
        let mut cfg = SettingsViewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn settings_view_config_remove() {
        let mut cfg = SettingsViewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn settings_view_config_keys_sorted() {
        let mut cfg = SettingsViewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn settings_view_config_bump_version() {
        let mut cfg = SettingsViewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn settings_view_config_clear() {
        let mut cfg = SettingsViewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn settings_view_config_merge() {
        let mut cfg1 = SettingsViewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SettingsViewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn settings_view_config_disable() {
        let mut cfg = SettingsViewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn settings_view_rate_tracker_empty() {
        let rt = SettingsViewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn settings_view_rate_tracker_record() {
        let mut rt = SettingsViewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn settings_view_rate_tracker_prune() {
        let mut rt = SettingsViewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn settings_view_validator_valid() {
        let v = SettingsViewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn settings_view_validator_errors() {
        let mut v = SettingsViewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn settings_view_validator_clear() {
        let mut v = SettingsViewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn settings_view_validator_merge() {
        let mut v1 = SettingsViewValidator::new();
        v1.add_error("e1");
        let mut v2 = SettingsViewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn settings_view_rate_tracker_clear() {
        let mut rt = SettingsViewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for settings_view
    #[test]
    fn xa_settings_view_ring_new() {
        let rb = super::XaSettingsViewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_settings_view_ring_push_len() {
        let mut rb = super::XaSettingsViewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_settings_view_ring_wrap() {
        let mut rb = super::XaSettingsViewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_settings_view_ring_mean_empty() {
        let rb = super::XaSettingsViewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_settings_view_ring_mean_values() {
        let mut rb = super::XaSettingsViewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_settings_view_ring_min_max() {
        let mut rb = super::XaSettingsViewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_settings_view_ring_iter() {
        let mut rb = super::XaSettingsViewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_settings_view_counter_new() {
        let c = super::XaSettingsViewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_settings_view_counter_inc() {
        let mut c = super::XaSettingsViewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_settings_view_counter_inc_by() {
        let mut c = super::XaSettingsViewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_settings_view_counter_reset() {
        let mut c = super::XaSettingsViewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_settings_view_counter_clear() {
        let mut c = super::XaSettingsViewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_settings_view_counter_default() {
        let c = super::XaSettingsViewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 158 ----

    #[test]
    fn xc_158_pool_new_empty() {
        let pool: super::Xc158Pool<i32> = super::Xc158Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_158_pool_release_acquire() {
        let mut pool = super::Xc158Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_158_pool_acquire_empty() {
        let mut pool: super::Xc158Pool<i32> = super::Xc158Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_158_pool_full() {
        let mut pool = super::Xc158Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_158_pool_drain() {
        let mut pool = super::Xc158Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_158_pool_stats() {
        let mut pool = super::Xc158Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_158_pool_clear() {
        let mut pool = super::Xc158Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_158_pool_shrink() {
        let mut pool = super::Xc158Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_158_pool_default() {
        let pool: super::Xc158Pool<String> = super::Xc158Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_158_pool_extend() {
        let mut pool = super::Xc158Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_158_pool_retain() {
        let mut pool = super::Xc158Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_158_scheduler_round_robin() {
        let mut sched = super::Xc158Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_158_scheduler_empty() {
        let mut sched = super::Xc158Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_158_scheduler_reset() {
        let mut sched = super::Xc158Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_158_scheduler_add_remove() {
        let mut sched = super::Xc158Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_158_scheduler_targets() {
        let sched = super::Xc158Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_158_hash_empty() {
        assert_eq!(super::xc_158_hash(b""), 5381);
    }

    #[test]
    fn xc_158_hash_data() {
        let h = super::xc_158_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_158_hash(b"hello"), h);
    }

    #[test]
    fn xc_158_reverse_str() {
        assert_eq!(super::xc_158_reverse("abc"), "cba");
        assert_eq!(super::xc_158_reverse(""), "");
    }


    // --- xd_60 deepening tests ---

    #[test]
    fn xd_60_sm_initial_state() {
        let sm = Xd60StateMachine::new();
        assert_eq!(sm.current_state(), Xd60State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_60_sm_valid_idle_to_running() {
        let mut sm = Xd60StateMachine::new();
        assert!(sm.transition(Xd60State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd60State::Running);
    }

    #[test]
    fn xd_60_sm_valid_running_to_paused() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        assert!(sm.transition(Xd60State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd60State::Paused);
    }

    #[test]
    fn xd_60_sm_valid_running_to_done() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        assert!(sm.transition(Xd60State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd60State::Done);
    }

    #[test]
    fn xd_60_sm_valid_paused_to_running() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        sm.transition(Xd60State::Paused).unwrap();
        assert!(sm.transition(Xd60State::Running).is_ok());
    }

    #[test]
    fn xd_60_sm_valid_done_to_idle() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        sm.transition(Xd60State::Done).unwrap();
        assert!(sm.transition(Xd60State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd60State::Idle);
    }

    #[test]
    fn xd_60_sm_invalid_idle_to_done() {
        let mut sm = Xd60StateMachine::new();
        assert!(sm.transition(Xd60State::Done).is_err());
    }

    #[test]
    fn xd_60_sm_invalid_idle_to_paused() {
        let mut sm = Xd60StateMachine::new();
        assert!(sm.transition(Xd60State::Paused).is_err());
    }

    #[test]
    fn xd_60_sm_history_tracking() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        sm.transition(Xd60State::Paused).unwrap();
        sm.transition(Xd60State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd60State::Idle);
        assert_eq!(sm.history()[0].to, Xd60State::Running);
        assert_eq!(sm.history()[1].from, Xd60State::Running);
        assert_eq!(sm.history()[2].to, Xd60State::Done);
    }

    #[test]
    fn xd_60_sm_serialize_deserialize() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd60StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd60State::Running));
    }

    #[test]
    fn xd_60_sm_deserialize_invalid() {
        assert_eq!(Xd60StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_60_sm_reset() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd60State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_60_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd60EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd60Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_60_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd60EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd60Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd60Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_60_bus_unsubscribe() {
        let mut bus = Xd60EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_60_event_kind_and_payload() {
        let e = Xd60Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd60Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_60_bus_clear_history() {
        let mut bus = Xd60EventBus::new();
        bus.publish(Xd60Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_60_sm_step_counter_increments() {
        let mut sm = Xd60StateMachine::new();
        sm.transition(Xd60State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd60State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #58 --

    #[test]
    fn xf58_trie_insert_search() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf58_trie_starts_with() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf58_trie_remove() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf58_trie_word_count() {
        let mut t = Xf58Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf58_trie_longest_prefix() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf58_trie_all_words() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf58_trie_autocomplete() {
        let mut t = Xf58Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf58_trie_empty_search() {
        let t = Xf58Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf58_bloom_add_contains() {
        let mut bf = Xf58BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf58_bloom_probably_absent() {
        let bf = Xf58BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf58_bloom_false_positive_rate() {
        let mut bf = Xf58BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf58_bloom_clear() {
        let mut bf = Xf58BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf58_bloom_union() {
        let mut a = Xf58BloomFilter::xf_new(512, 2);
        let mut b = Xf58BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf58_bloom_intersection_estimate() {
        let mut a = Xf58BloomFilter::xf_new(512, 2);
        let mut b = Xf58BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf58_bloom_union_size_mismatch() {
        let a = Xf58BloomFilter::xf_new(256, 2);
        let b = Xf58BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh157_skip_insert_contains() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh157_skip_remove() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh157_skip_len() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh157_skip_range_query() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh157_skip_floor_ceiling() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh157_skip_rank() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh157_skip_empty() {
        let sl = super::Xh157SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh157_skip_duplicates() {
        let mut sl = super::Xh157SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh157_bitset_set_test() {
        let mut bs = super::Xh157BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh157_bitset_clear_count() {
        let mut bs = super::Xh157BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh157_bitset_and_or_xor() {
        let mut a = super::Xh157BitSet::xh_new(128);
        let mut b = super::Xh157BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh157_bitset_iter_ones() {
        let mut bs = super::Xh157BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh157_bitset_first_last() {
        let mut bs = super::Xh157BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh157_bitset_empty() {
        let bs = super::Xh157BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi157_deque_push_pop_back() {
        let mut dq = super::Xi157Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi157_deque_push_pop_front() {
        let mut dq = super::Xi157Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi157_deque_mixed_ops() {
        let mut dq = super::Xi157Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi157_deque_get_and_split() {
        let mut dq = super::Xi157Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi157_deque_rotate_left() {
        let mut dq = super::Xi157Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi157_deque_rotate_right() {
        let mut dq = super::Xi157Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi157_deque_grow() {
        let mut dq = super::Xi157Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi157_deque_empty() {
        let dq = super::Xi157Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi157_interval_tree_insert_query() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi157Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi157Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi157_interval_tree_overlap() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi157Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi157Interval::xi_new(12, 20));
        let q = super::Xi157Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi157_interval_tree_remove() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi157Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi157_interval_tree_gaps() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi157Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi157Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi157Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi157Interval::xi_new(8, 10));
    }

    #[test]
    fn xi157_interval_tree_merge() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi157Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi157Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi157Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi157Interval::xi_new(10, 15));
    }

    #[test]
    fn xi157_interval_tree_all() {
        let mut tree = super::Xi157IntervalTree::xi_new();
        tree.xi_insert(super::Xi157Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi157Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi157_interval_tree_empty() {
        let tree = super::Xi157IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi157_interval_tree_contains_point() {
        let iv = super::Xi157Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 157) ---

    #[test]
    fn xj_157_uf_make_and_find() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_157_uf_union_connected() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_157_uf_component_count() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_157_uf_component_size() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_157_uf_largest_component() {
        let mut uf = super::Xj157UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_157_uf_many_elements() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_157_uf_separate_components() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_157_uf_path_compression() {
        let mut uf = super::Xj157UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_157_bt_insert_get() {
        let mut bt = super::Xj157BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_157_bt_contains_len() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_157_bt_replace() {
        let mut bt = super::Xj157BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_157_bt_remove() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_157_bt_keys_values() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_157_bt_range() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_157_bt_min_max() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_157_bt_many_inserts() {
        let mut bt = super::Xj157BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_157 segment tree tests ---

    #[test]
    fn xk_157_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_157_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk157SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_157_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_157_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_157_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_157_st_single_element() {
        let data = vec![42];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_157_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk157SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_157_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk157SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_157 disjoint intervals tests ---

    #[test]
    fn xk_157_di_add_and_count() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_157_di_merge_overlap() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_157_di_contains() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_157_di_remove() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_157_di_covered_length() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_157_di_gaps() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_157_di_merge_adjacent() {
        let mut di = super::Xk157DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_157_di_empty() {
        let di = super::Xk157DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
