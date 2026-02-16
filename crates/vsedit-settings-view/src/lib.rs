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
}
