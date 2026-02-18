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

/// A group of related status bar items.
#[derive(Debug, Clone)]
pub struct StatusBarGroup {
    pub group_id: String,
    pub entry_ids: Vec<String>,
}

impl StatusBarGroup {
    /// Create a new empty group.
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            entry_ids: Vec::new(),
        }
    }

    /// Add an entry ID to the group.
    pub fn add(&mut self, entry_id: impl Into<String>) {
        self.entry_ids.push(entry_id.into());
    }

    /// Check if the group contains a given entry ID.
    pub fn contains(&self, entry_id: &str) -> bool {
        self.entry_ids.iter().any(|id| id == entry_id)
    }

    /// Number of entries in the group.
    pub fn len(&self) -> usize {
        self.entry_ids.len()
    }

    /// Returns true if the group is empty.
    pub fn is_empty(&self) -> bool {
        self.entry_ids.is_empty()
    }
}

/// Layout metrics for a status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarLayout {
    pub left_count: usize,
    pub right_count: usize,
    pub total_visible: usize,
    pub total_hidden: usize,
    pub left_text_width: usize,
    pub right_text_width: usize,
}

impl StatusBar {
    /// Compute layout metrics for the current status bar state.
    pub fn compute_layout(&self) -> StatusBarLayout {
        let mut left_count = 0;
        let mut right_count = 0;
        let mut total_visible = 0;
        let mut total_hidden = 0;
        let mut left_text_width = 0;
        let mut right_text_width = 0;

        for entry in &self.entries {
            if entry.visible {
                total_visible += 1;
                match entry.alignment {
                    StatusBarAlignment::Left => {
                        left_count += 1;
                        left_text_width += entry.text.len();
                    }
                    StatusBarAlignment::Right => {
                        right_count += 1;
                        right_text_width += entry.text.len();
                    }
                }
            } else {
                total_hidden += 1;
            }
        }

        StatusBarLayout {
            left_count,
            right_count,
            total_visible,
            total_hidden,
            left_text_width,
            right_text_width,
        }
    }

    /// Set visibility for all entries in the given group.
    pub fn set_group_visibility(&mut self, group: &StatusBarGroup, visible: bool) {
        for entry_id in &group.entry_ids {
            self.set_visibility(entry_id, visible);
        }
    }

    /// Get all tooltips currently set on visible entries.
    pub fn collect_tooltips(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter(|e| e.visible && e.tooltip.is_some())
            .map(|e| (e.id.as_str(), e.tooltip.as_deref().unwrap()))
            .collect()
    }

    /// Clear all tooltips from all entries.
    pub fn clear_tooltips(&mut self) {
        for entry in &mut self.entries {
            entry.tooltip = None;
        }
    }

    /// Toggle visibility for all entries in the bar.
    pub fn toggle_all_visibility(&mut self) {
        for entry in &mut self.entries {
            entry.visible = !entry.visible;
        }
    }
}

// ---------------------------------------------------------------------------
// Priority-sorted rendering and tooltip formatting
// ---------------------------------------------------------------------------

/// Priority tier for status bar entries, finer-grained than the i32 priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatusBarPriorityTier {
    /// Always shown, highest priority.
    Essential,
    /// Shown by default but can be hidden.
    Standard,
    /// Only shown when space allows.
    Optional,
}

impl fmt::Display for StatusBarPriorityTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarPriorityTier::Essential => write!(f, "Essential"),
            StatusBarPriorityTier::Standard => write!(f, "Standard"),
            StatusBarPriorityTier::Optional => write!(f, "Optional"),
        }
    }
}

/// A rendered tooltip with optional markdown-like formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarTooltip {
    pub entry_id: String,
    pub title: String,
    pub description: Option<String>,
    pub shortcut: Option<String>,
}

impl StatusBarTooltip {
    pub fn new(entry_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            entry_id: entry_id.into(),
            title: title.into(),
            description: None,
            shortcut: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Render the tooltip to a formatted string.
    pub fn render(&self) -> String {
        let mut out = self.title.clone();
        if let Some(ref desc) = self.description {
            out.push_str("\n");
            out.push_str(desc);
        }
        if let Some(ref shortcut) = self.shortcut {
            out.push_str(&format!("\n({})", shortcut));
        }
        out
    }
}

impl fmt::Display for StatusBarTooltip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

/// Visibility rule for a status bar entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarVisibility {
    /// Always visible.
    Always,
    /// Hidden unless explicitly shown.
    Hidden,
    /// Visible only when the entry has content (non-empty text).
    WhenNonEmpty,
}

impl StatusBar {
    /// Render entries sorted by priority within each alignment, producing
    /// a pair of (left_text, right_text) with entries separated by `separator`.
    pub fn render_with_separator(&self, separator: &str) -> (String, String) {
        let mut left: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == StatusBarAlignment::Left)
            .collect();
        left.sort_by_key(|e| e.priority);
        let left_text = left.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join(separator);

        let mut right: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == StatusBarAlignment::Right)
            .collect();
        right.sort_by_key(|e| e.priority);
        let right_text = right.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join(separator);

        (left_text, right_text)
    }

    /// Apply a visibility rule to an entry.
    pub fn apply_visibility_rule(&mut self, id: &str, rule: StatusBarVisibility) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            match rule {
                StatusBarVisibility::Always => entry.visible = true,
                StatusBarVisibility::Hidden => entry.visible = false,
                StatusBarVisibility::WhenNonEmpty => entry.visible = !entry.text.is_empty(),
            }
            true
        } else {
            false
        }
    }

    /// Render tooltips for all visible entries that have tooltips set.
    pub fn render_tooltips(&self) -> Vec<StatusBarTooltip> {
        self.entries
            .iter()
            .filter(|e| e.visible && e.tooltip.is_some())
            .map(|e| StatusBarTooltip::new(&e.id, e.tooltip.as_deref().unwrap_or("")))
            .collect()
    }

    /// Get entries sorted by priority tier, where priority < 0 is Essential,
    /// 0..=50 is Standard, and > 50 is Optional.
    pub fn entries_by_tier(&self) -> Vec<(&StatusBarEntry, StatusBarPriorityTier)> {
        let mut result: Vec<(&StatusBarEntry, StatusBarPriorityTier)> = self
            .entries
            .iter()
            .map(|e| {
                let tier = if e.priority < 0 {
                    StatusBarPriorityTier::Essential
                } else if e.priority <= 50 {
                    StatusBarPriorityTier::Standard
                } else {
                    StatusBarPriorityTier::Optional
                };
                (e, tier)
            })
            .collect();
        result.sort_by_key(|(_, tier)| *tier);
        result
    }

    /// Hide all optional entries (priority > 50).
    pub fn hide_optional_entries(&mut self) {
        for entry in &mut self.entries {
            if entry.priority > 50 {
                entry.visible = false;
            }
        }
    }

    /// Show all essential entries (priority < 0).
    pub fn show_essential_entries(&mut self) {
        for entry in &mut self.entries {
            if entry.priority < 0 {
                entry.visible = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry query helpers
// ---------------------------------------------------------------------------

impl StatusBarEntry {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn has_tooltip(&self) -> bool {
        self.tooltip.is_some()
    }

    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.id.to_lowercase().contains(&q) || self.text.to_lowercase().contains(&q)
    }
}

// ---------------------------------------------------------------------------
// StatusBar iteration and query extensions
// ---------------------------------------------------------------------------

impl StatusBar {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn visible_entries(&self) -> Vec<&StatusBarEntry> {
        self.entries.iter().filter(|e| e.visible).collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<&StatusBarEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, StatusBarEntry> {
        self.entries.iter()
    }

    pub fn summary(&self) -> StatusBarSummary {
        let total = self.entries.len();
        let visible = self.entries.iter().filter(|e| e.visible).count();
        let with_tooltip = self.entries.iter().filter(|e| e.tooltip.is_some()).count();
        let with_command = self.entries.iter().filter(|e| e.command.is_some()).count();
        let left = self
            .entries
            .iter()
            .filter(|e| e.alignment == StatusBarAlignment::Left)
            .count();
        let right = total - left;
        StatusBarSummary {
            total,
            visible,
            hidden: total - visible,
            left,
            right,
            with_tooltip,
            with_command,
        }
    }
}

impl<'a> IntoIterator for &'a StatusBar {
    type Item = &'a StatusBarEntry;
    type IntoIter = std::slice::Iter<'a, StatusBarEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

// ---------------------------------------------------------------------------
// StatusBarSnapshot extensions
// ---------------------------------------------------------------------------

impl StatusBarSnapshot {
    pub fn diff(&self, other: &StatusBarSnapshot) -> Vec<String> {
        let mut changes = Vec::new();
        for entry in &self.entries {
            match other.get_entry(&entry.id) {
                None => changes.push(format!("removed: {}", entry.id)),
                Some(other_entry) => {
                    if entry.text != other_entry.text {
                        changes.push(format!(
                            "changed text: {} '{}' -> '{}'",
                            entry.id, entry.text, other_entry.text
                        ));
                    }
                    if entry.visible != other_entry.visible {
                        changes.push(format!(
                            "changed visibility: {} {} -> {}",
                            entry.id, entry.visible, other_entry.visible
                        ));
                    }
                }
            }
        }
        for entry in &other.entries {
            if self.get_entry(&entry.id).is_none() {
                changes.push(format!("added: {}", entry.id));
            }
        }
        changes
    }
}

impl fmt::Display for StatusBarSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatusBarSnapshot({} entries)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// StatusBarGroup extensions
// ---------------------------------------------------------------------------

impl StatusBarGroup {
    pub fn entry_count(&self) -> usize {
        self.entry_ids.len()
    }

    pub fn merge(&mut self, other: &StatusBarGroup) {
        for id in &other.entry_ids {
            if !self.contains(id) {
                self.entry_ids.push(id.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// StatusBarLayout extensions
// ---------------------------------------------------------------------------

impl StatusBarLayout {
    pub fn total_width(&self) -> usize {
        self.left_text_width + self.right_text_width
    }

    pub fn left_count(&self) -> usize {
        self.left_count
    }

    pub fn right_count(&self) -> usize {
        self.right_count
    }
}

// ---------------------------------------------------------------------------
// StatusBarPriorityTier extensions
// ---------------------------------------------------------------------------

impl StatusBarPriorityTier {
    pub fn is_high(&self) -> bool {
        *self == StatusBarPriorityTier::Essential
    }

    pub fn label(&self) -> &'static str {
        match self {
            StatusBarPriorityTier::Essential => "essential",
            StatusBarPriorityTier::Standard => "standard",
            StatusBarPriorityTier::Optional => "optional",
        }
    }
}

// ---------------------------------------------------------------------------
// StatusBarTooltip extensions
// ---------------------------------------------------------------------------

impl StatusBarTooltip {
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.description.is_none() && self.shortcut.is_none()
    }

    pub fn word_count(&self) -> usize {
        let mut count = self.title.split_whitespace().count();
        if let Some(ref desc) = self.description {
            count += desc.split_whitespace().count();
        }
        if let Some(ref shortcut) = self.shortcut {
            count += shortcut.split_whitespace().count();
        }
        count
    }
}

// ---------------------------------------------------------------------------
// StatusBarVisibility extensions
// ---------------------------------------------------------------------------

impl StatusBarVisibility {
    pub fn is_shown(&self) -> bool {
        matches!(self, StatusBarVisibility::Always)
    }
}

impl fmt::Display for StatusBarVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarVisibility::Always => write!(f, "Always"),
            StatusBarVisibility::Hidden => write!(f, "Hidden"),
            StatusBarVisibility::WhenNonEmpty => write!(f, "WhenNonEmpty"),
        }
    }
}

// ---------------------------------------------------------------------------
// Animation states for status bar items
// ---------------------------------------------------------------------------

/// Animation state for a status bar entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// No animation, entry is static.
    Idle,
    /// Entry is fading in (e.g., just appeared).
    FadingIn,
    /// Entry is fading out (e.g., about to be removed).
    FadingOut,
    /// Entry is pulsing to draw attention (e.g., new notification).
    Pulsing,
    /// Entry content is spinning (e.g., a progress indicator).
    Spinning,
}

impl fmt::Display for AnimationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimationState::Idle => write!(f, "Idle"),
            AnimationState::FadingIn => write!(f, "FadingIn"),
            AnimationState::FadingOut => write!(f, "FadingOut"),
            AnimationState::Pulsing => write!(f, "Pulsing"),
            AnimationState::Spinning => write!(f, "Spinning"),
        }
    }
}

impl AnimationState {
    /// Returns true if the entry is currently animating.
    pub fn is_animating(&self) -> bool {
        !matches!(self, AnimationState::Idle)
    }

    /// Returns true if the animation represents a transition (fade in/out).
    pub fn is_transition(&self) -> bool {
        matches!(self, AnimationState::FadingIn | AnimationState::FadingOut)
    }
}

// ---------------------------------------------------------------------------
// Click action routing
// ---------------------------------------------------------------------------

/// Describes what happens when a status bar entry is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Execute a named command.
    RunCommand(String),
    /// Open a URL in the browser.
    OpenUrl(String),
    /// Show a quick-pick menu with the given options.
    ShowMenu(Vec<String>),
    /// No action configured.
    None,
}

impl ClickAction {
    /// Returns true if a click action is configured.
    pub fn is_actionable(&self) -> bool {
        !matches!(self, ClickAction::None)
    }

    /// Returns the command name if this is a `RunCommand` action.
    pub fn command_name(&self) -> Option<&str> {
        match self {
            ClickAction::RunCommand(cmd) => Some(cmd),
            _ => None,
        }
    }
}

impl fmt::Display for ClickAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickAction::RunCommand(cmd) => write!(f, "command:{cmd}"),
            ClickAction::OpenUrl(url) => write!(f, "url:{url}"),
            ClickAction::ShowMenu(items) => write!(f, "menu[{}]", items.len()),
            ClickAction::None => write!(f, "none"),
        }
    }
}

// ---------------------------------------------------------------------------
// Space allocation algorithm
// ---------------------------------------------------------------------------

/// Result of allocating available width across status bar entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceAllocation {
    /// Entry IDs that fit within the available width, in display order.
    pub displayed: Vec<String>,
    /// Entry IDs that were truncated or hidden due to insufficient space.
    pub overflowed: Vec<String>,
    /// Total character width consumed by displayed entries (including separators).
    pub consumed_width: usize,
    /// Remaining width after allocation.
    pub remaining_width: usize,
}

impl StatusBar {
    /// Allocate entries into available width for a given alignment.
    ///
    /// Entries are considered in priority order (ascending). Each entry consumes
    /// `entry.text.len() + separator_width` characters (the last entry doesn't
    /// add separator). Entries that don't fit go into the overflow list.
    pub fn allocate_space(
        &self,
        alignment: StatusBarAlignment,
        available_width: usize,
        separator_width: usize,
    ) -> SpaceAllocation {
        let mut entries: Vec<&StatusBarEntry> = self
            .entries
            .iter()
            .filter(|e| e.visible && e.alignment == alignment)
            .collect();
        entries.sort_by_key(|e| e.priority);

        let mut displayed = Vec::new();
        let mut overflowed = Vec::new();
        let mut consumed: usize = 0;

        for (i, entry) in entries.iter().enumerate() {
            let sep = if i > 0 && !displayed.is_empty() {
                separator_width
            } else {
                0
            };
            let needed = entry.text.len() + sep;
            if consumed + needed <= available_width {
                consumed += needed;
                displayed.push(entry.id.clone());
            } else {
                overflowed.push(entry.id.clone());
            }
        }

        SpaceAllocation {
            displayed,
            overflowed,
            consumed_width: consumed,
            remaining_width: available_width.saturating_sub(consumed),
        }
    }

    /// Route a click event to the appropriate action for the given entry.
    ///
    /// If the entry has a `command` field set, returns `ClickAction::RunCommand`.
    /// Otherwise returns `ClickAction::None`.
    pub fn route_click(&self, entry_id: &str) -> ClickAction {
        match self.entries.iter().find(|e| e.id == entry_id) {
            Some(entry) => match &entry.command {
                Some(cmd) => ClickAction::RunCommand(cmd.clone()),
                None => ClickAction::None,
            },
            None => ClickAction::None,
        }
    }

    /// Generate a rich tooltip for an entry, combining its tooltip text with
    /// contextual information (command binding, alignment, priority tier).
    pub fn generate_tooltip(&self, entry_id: &str) -> Option<StatusBarTooltip> {
        let entry = self.entries.iter().find(|e| e.id == entry_id)?;
        let title = entry
            .tooltip
            .clone()
            .unwrap_or_else(|| entry.text.clone());
        let tier = if entry.priority < 0 {
            StatusBarPriorityTier::Essential
        } else if entry.priority <= 50 {
            StatusBarPriorityTier::Standard
        } else {
            StatusBarPriorityTier::Optional
        };
        let description = format!(
            "Alignment: {} | Priority: {} ({})",
            entry.alignment, entry.priority, tier
        );
        let mut tip = StatusBarTooltip::new(entry_id, title).with_description(description);
        if let Some(ref cmd) = entry.command {
            tip = tip.with_shortcut(cmd.clone());
        }
        Some(tip)
    }

    /// Return IDs of entries that are currently overflowing the given width
    /// (considering both left and right sides, each getting half the width).
    pub fn overflow_entries(&self, total_width: usize, separator_width: usize) -> Vec<String> {
        let half = total_width / 2;
        let left = self.allocate_space(StatusBarAlignment::Left, half, separator_width);
        let right = self.allocate_space(StatusBarAlignment::Right, half, separator_width);
        let mut overflow = left.overflowed;
        overflow.extend(right.overflowed);
        overflow
    }
}

// ---------------------------------------------------------------------------
// Statistical summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarSummary {
    pub total: usize,
    pub visible: usize,
    pub hidden: usize,
    pub left: usize,
    pub right: usize,
    pub with_tooltip: usize,
    pub with_command: usize,
}

impl fmt::Display for StatusBarSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "total={} visible={} hidden={} left={} right={} tooltips={} commands={}",
            self.total,
            self.visible,
            self.hidden,
            self.left,
            self.right,
            self.with_tooltip,
            self.with_command,
        )
    }
}

// --- StatusBarAlignmentPriority: ordering struct for sorted display ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarAlignmentPriority {
    pub alignment: StatusBarAlignment,
    pub priority: i32,
    pub item_id: String,
}

impl StatusBarAlignmentPriority {
    pub fn new(
        alignment: StatusBarAlignment,
        priority: i32,
        item_id: impl Into<String>,
    ) -> Self {
        Self {
            alignment,
            priority,
            item_id: item_id.into(),
        }
    }

    /// Returns items sorted left-before-right, then by priority ascending.
    pub fn sorted_items(items: &[StatusBarAlignmentPriority]) -> Vec<StatusBarAlignmentPriority> {
        let mut sorted = items.to_vec();
        sorted.sort();
        sorted
    }
}

impl Ord for StatusBarAlignmentPriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let align_ord = |a: &StatusBarAlignment| -> u8 {
            match a {
                StatusBarAlignment::Left => 0,
                StatusBarAlignment::Right => 1,
            }
        };
        align_ord(&self.alignment)
            .cmp(&align_ord(&other.alignment))
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| self.item_id.cmp(&other.item_id))
    }
}

impl PartialOrd for StatusBarAlignmentPriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// --- StatusBarItemGroup: logical grouping of status bar items ---

#[derive(Debug, Clone)]
pub struct StatusBarItemGroup {
    pub group_id: String,
    pub item_ids: Vec<String>,
    pub collapsed: bool,
}

impl StatusBarItemGroup {
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            item_ids: Vec::new(),
            collapsed: false,
        }
    }

    pub fn add_item(&mut self, item_id: impl Into<String>) {
        let id = item_id.into();
        if !self.item_ids.contains(&id) {
            self.item_ids.push(id);
        }
    }

    pub fn remove_item(&mut self, item_id: &str) -> bool {
        let len_before = self.item_ids.len();
        self.item_ids.retain(|id| id != item_id);
        self.item_ids.len() != len_before
    }

    pub fn toggle_collapse(&mut self) {
        self.collapsed = !self.collapsed;
    }

    pub fn item_count(&self) -> usize {
        self.item_ids.len()
    }

    pub fn contains(&self, item_id: &str) -> bool {
        self.item_ids.iter().any(|id| id == item_id)
    }
}

impl fmt::Display for StatusBarItemGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.collapsed {
            "collapsed"
        } else {
            "expanded"
        };
        write!(
            f,
            "Group '{}' ({}, {} items)",
            self.group_id,
            state,
            self.item_ids.len()
        )
    }
}

// --- StatusBarTooltipBuilder: builder for rich tooltips ---

#[derive(Debug, Clone)]
pub struct StatusBarTooltipBuilder {
    title: String,
    body_lines: Vec<String>,
    links: Vec<(String, String)>,
}

impl StatusBarTooltipBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body_lines: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn add_line(mut self, line: impl Into<String>) -> Self {
        self.body_lines.push(line.into());
        self
    }

    pub fn add_link(mut self, label: impl Into<String>, url: impl Into<String>) -> Self {
        self.links.push((label.into(), url.into()));
        self
    }

    /// Builds a `StatusBarTooltip` whose `title` field is the builder title
    /// and whose `description` is the rendered body lines + links.
    pub fn build(self, entry_id: impl Into<String>) -> StatusBarTooltip {
        let mut text_parts: Vec<String> = Vec::new();
        for line in &self.body_lines {
            text_parts.push(line.clone());
        }
        for (label, url) in &self.links {
            text_parts.push(format!("[{}]({})", label, url));
        }
        let description = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        };
        StatusBarTooltip {
            entry_id: entry_id.into(),
            title: self.title,
            description,
            shortcut: None,
        }
    }
}

// --- StatusBarItemToggle: visibility toggle with counter ---

#[derive(Debug, Clone)]
pub struct StatusBarItemToggle {
    pub item_id: String,
    pub visible: bool,
    pub toggle_count: u32,
}

impl StatusBarItemToggle {
    pub fn new(item_id: impl Into<String>) -> Self {
        Self {
            item_id: item_id.into(),
            visible: true,
            toggle_count: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.toggle_count += 1;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn reset(&mut self) {
        self.visible = true;
        self.toggle_count = 0;
    }
}

// ---------------------------------------------------------------------------
// StatusBarAnimationTick – animate status bar elements
// ---------------------------------------------------------------------------

/// Frame data for a status bar animation (e.g. a spinner).
#[derive(Debug, Clone)]
pub struct StatusBarAnimationTick {
    frames: Vec<String>,
    current_frame: usize,
    interval_ms: u64,
    elapsed_ms: u64,
    running: bool,
}

impl StatusBarAnimationTick {
    pub fn new(frames: Vec<String>, interval_ms: u64) -> Self {
        Self {
            frames,
            current_frame: 0,
            interval_ms,
            elapsed_ms: 0,
            running: false,
        }
    }

    /// Create a spinner animation with default frames.
    pub fn spinner(interval_ms: u64) -> Self {
        Self::new(
            vec![
                "⠋".into(), "⠙".into(), "⠹".into(), "⠸".into(),
                "⠼".into(), "⠴".into(), "⠦".into(), "⠧".into(),
                "⠇".into(), "⠏".into(),
            ],
            interval_ms,
        )
    }

    /// Create a dots animation.
    pub fn dots(interval_ms: u64) -> Self {
        Self::new(
            vec![".".into(), "..".into(), "...".into(), "".into()],
            interval_ms,
        )
    }

    pub fn start(&mut self) {
        self.running = true;
        self.elapsed_ms = 0;
        self.current_frame = 0;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Advance the animation by `delta_ms` milliseconds. Returns true if the frame changed.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.running || self.frames.is_empty() {
            return false;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.interval_ms {
            self.elapsed_ms -= self.interval_ms;
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            true
        } else {
            false
        }
    }

    /// Get the current frame string.
    pub fn current_frame_str(&self) -> &str {
        if self.frames.is_empty() {
            return "";
        }
        &self.frames[self.current_frame]
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn current_index(&self) -> usize {
        self.current_frame
    }

    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.elapsed_ms = 0;
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    pub fn set_interval_ms(&mut self, ms: u64) {
        self.interval_ms = ms;
    }
}

// ---------------------------------------------------------------------------
// StatusBarSeparatorRenderer – render separators between items
// ---------------------------------------------------------------------------

/// Style of separator between status bar items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorStyle {
    Pipe,
    Dot,
    Dash,
    Space,
    Custom(char),
}

impl SeparatorStyle {
    pub fn as_str(&self) -> String {
        match self {
            Self::Pipe => " | ".to_string(),
            Self::Dot => " · ".to_string(),
            Self::Dash => " - ".to_string(),
            Self::Space => "  ".to_string(),
            Self::Custom(c) => format!(" {c} "),
        }
    }
}

impl fmt::Display for SeparatorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Renders separators between status bar items.
#[derive(Debug, Clone)]
pub struct StatusBarSeparatorRenderer {
    left_style: SeparatorStyle,
    right_style: SeparatorStyle,
}

impl StatusBarSeparatorRenderer {
    pub fn new(style: SeparatorStyle) -> Self {
        Self {
            left_style: style,
            right_style: style,
        }
    }

    pub fn with_different_sides(left: SeparatorStyle, right: SeparatorStyle) -> Self {
        Self {
            left_style: left,
            right_style: right,
        }
    }

    pub fn left_style(&self) -> SeparatorStyle {
        self.left_style
    }

    pub fn right_style(&self) -> SeparatorStyle {
        self.right_style
    }

    /// Join a list of entry texts with the appropriate separator for the given alignment.
    pub fn join_texts(&self, texts: &[&str], alignment: StatusBarAlignment) -> String {
        let sep = match alignment {
            StatusBarAlignment::Left => self.left_style.as_str(),
            StatusBarAlignment::Right => self.right_style.as_str(),
        };
        texts.join(&sep)
    }

    /// Render left and right sides into a full status bar string with a given total width.
    pub fn render_bar(&self, left_items: &[&str], right_items: &[&str], total_width: usize) -> String {
        let left = self.join_texts(left_items, StatusBarAlignment::Left);
        let right = self.join_texts(right_items, StatusBarAlignment::Right);
        let padding = total_width.saturating_sub(left.len() + right.len());
        format!("{}{}{}", left, " ".repeat(padding), right)
    }
}

impl Default for StatusBarSeparatorRenderer {
    fn default() -> Self {
        Self::new(SeparatorStyle::Pipe)
    }
}

// ---------------------------------------------------------------------------
// StatusBarItemPriority – manage item ordering by priority
// ---------------------------------------------------------------------------

/// A priority-sortable wrapper around a status bar entry ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarItemPriority {
    pub entry_id: String,
    pub priority: i32,
    pub alignment: StatusBarAlignment,
}

/// Manages priority ordering for status bar entries.
#[derive(Debug)]
pub struct StatusBarPriorityManager {
    items: Vec<StatusBarItemPriority>,
}

impl StatusBarPriorityManager {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn register(&mut self, entry_id: impl Into<String>, priority: i32, alignment: StatusBarAlignment) {
        let id = entry_id.into();
        if let Some(existing) = self.items.iter_mut().find(|i| i.entry_id == id) {
            existing.priority = priority;
            existing.alignment = alignment;
        } else {
            self.items.push(StatusBarItemPriority {
                entry_id: id,
                priority,
                alignment,
            });
        }
    }

    pub fn unregister(&mut self, entry_id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.entry_id != entry_id);
        self.items.len() < before
    }

    /// Get ordered entry IDs for one side, sorted by descending priority.
    pub fn ordered_ids(&self, alignment: StatusBarAlignment) -> Vec<&str> {
        let mut side: Vec<&StatusBarItemPriority> = self
            .items
            .iter()
            .filter(|i| i.alignment == alignment)
            .collect();
        side.sort_by(|a, b| b.priority.cmp(&a.priority));
        side.iter().map(|i| i.entry_id.as_str()).collect()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn get_priority(&self, entry_id: &str) -> Option<i32> {
        self.items.iter().find(|i| i.entry_id == entry_id).map(|i| i.priority)
    }

    pub fn set_priority(&mut self, entry_id: &str, priority: i32) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.entry_id == entry_id) {
            item.priority = priority;
            true
        } else {
            false
        }
    }

    /// Find the highest-priority item on a given side.
    pub fn highest_priority(&self, alignment: StatusBarAlignment) -> Option<&StatusBarItemPriority> {
        self.items
            .iter()
            .filter(|i| i.alignment == alignment)
            .max_by_key(|i| i.priority)
    }
}

impl Default for StatusBarPriorityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StatusBarBackgroundColorManager – manage item background colors
// ---------------------------------------------------------------------------

/// Manages background colors for status bar regions.
#[derive(Debug, Clone)]
pub struct StatusBarBackgroundColorManager {
    default_color: String,
    overrides: Vec<(String, String)>,
    error_color: String,
    warning_color: String,
}

impl StatusBarBackgroundColorManager {
    pub fn new(default_color: impl Into<String>) -> Self {
        Self {
            default_color: default_color.into(),
            overrides: Vec::new(),
            error_color: "#e51400".to_string(),
            warning_color: "#c8a000".to_string(),
        }
    }

    pub fn default_color(&self) -> &str {
        &self.default_color
    }

    pub fn set_default_color(&mut self, color: impl Into<String>) {
        self.default_color = color.into();
    }

    pub fn error_color(&self) -> &str {
        &self.error_color
    }

    pub fn set_error_color(&mut self, color: impl Into<String>) {
        self.error_color = color.into();
    }

    pub fn warning_color(&self) -> &str {
        &self.warning_color
    }

    pub fn set_warning_color(&mut self, color: impl Into<String>) {
        self.warning_color = color.into();
    }

    /// Set an override color for a specific entry.
    pub fn set_override(&mut self, entry_id: impl Into<String>, color: impl Into<String>) {
        let id = entry_id.into();
        let color = color.into();
        if let Some(existing) = self.overrides.iter_mut().find(|(eid, _)| *eid == id) {
            existing.1 = color;
        } else {
            self.overrides.push((id, color));
        }
    }

    /// Remove an override for a specific entry.
    pub fn remove_override(&mut self, entry_id: &str) -> bool {
        let before = self.overrides.len();
        self.overrides.retain(|(eid, _)| eid != entry_id);
        self.overrides.len() < before
    }

    /// Resolve the background color for an entry, checking overrides first.
    pub fn resolve_color(&self, entry_id: &str) -> &str {
        self.overrides
            .iter()
            .find(|(eid, _)| eid == entry_id)
            .map(|(_, c)| c.as_str())
            .unwrap_or(&self.default_color)
    }

    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Clear all overrides.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    /// Resolve a color based on whether there are errors or warnings.
    pub fn resolve_status_color(&self, has_errors: bool, has_warnings: bool) -> &str {
        if has_errors {
            &self.error_color
        } else if has_warnings {
            &self.warning_color
        } else {
            &self.default_color
        }
    }
}

impl Default for StatusBarBackgroundColorManager {
    fn default() -> Self {
        Self::new("#007acc")
    }
}


// ---------------------------------------------------------------------------
// statusbar – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XStatusbarLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XStatusbarPanelState {
    pub region: XStatusbarLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XStatusbarPanelState {
    pub fn new(region: XStatusbarLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_statusbar_total_visible_area(panels: &[XStatusbarPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_statusbar_count_in_region(
    panels: &[XStatusbarPanelState],
    region: XStatusbarLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_statusbar_widest_panel(panels: &[XStatusbarPanelState]) -> Option<&XStatusbarPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_statusbar_collapse_region(
    panels: &mut [XStatusbarPanelState],
    region: XStatusbarLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XStatusbarLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XStatusbarLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// statusbar – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for status bar items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YStatusbarStatusbarAlignment {
    Left,
    Right,
    Center,
    Hidden,
}

impl YStatusbarStatusbarAlignment {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Center => 2,
            Self::Hidden => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Center => "Center",
            Self::Hidden => "Hidden",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YStatusbarStatusbarAlignment] {
        &[
            YStatusbarStatusbarAlignment::Left,
            YStatusbarStatusbarAlignment::Right,
            YStatusbarStatusbarAlignment::Center,
            YStatusbarStatusbarAlignment::Hidden,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YStatusbarStatusbarAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks statusbar item data.
#[derive(Debug, Clone)]
pub struct YStatusbarStatusbarItem {
    pub id: String,
    pub text: String,
    pub priority: i32,
}

impl YStatusbarStatusbarItem {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            id: String::new(),
            text: String::new(),
            priority: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YStatusbarStatusbarItem({}: {:?})", "id", self.id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_statusbar_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_statusbar_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_statusbar_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_statusbar_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_statusbar_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_statusbar_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_statusbar_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_statusbar_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// statusbar – Extended statusbar animation helpers
// ---------------------------------------------------------------------------

/// Priority levels for statusbar animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZStatusbarPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZStatusbarPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZStatusbarPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZStatusbarPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks statusbar animation data.
#[derive(Debug, Clone)]
pub struct ZStatusbarStatusbarAnimation {
    pub frames: Vec<String>,
    pub interval_ms: u64,
    pub looping: bool,
}

impl ZStatusbarStatusbarAnimation {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            interval_ms: 0,
            looping: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZStatusbarStatusbarAnimation[interval_ms={:?}, looping={:?}]", self.interval_ms, self.looping)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.looping = !c.looping;
        c
    }
}

/// Compute a simple rolling hash for statusbar animation.
pub fn z_statusbar_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_statusbar_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_statusbar_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_statusbar_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_statusbar_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_statusbar_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_statusbar_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 68
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer68 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer68 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_68(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_68<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_68<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_68(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_68(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 164
// ---------------------------------------------------------------------------

/// Generic object pool `Xc164Pool<T>`.
pub struct Xc164Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc164Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc164PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc164Pool<T> {
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
    pub fn stats(&self) -> Xc164PoolStats {
        Xc164PoolStats {
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

impl<T> Default for Xc164Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc164Scheduler`.
pub struct Xc164Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc164Scheduler {
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

impl Default for Xc164Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_164 hash for the given byte slice.
pub fn xc_164_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_164 convention.
pub fn xc_164_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    #[test]
    fn status_bar_group_basic() {
        let mut group = StatusBarGroup::new("git-group");
        assert!(group.is_empty());
        group.add("branch");
        group.add("status");
        assert_eq!(group.len(), 2);
        assert!(group.contains("branch"));
        assert!(!group.contains("missing"));
    }

    #[test]
    fn compute_layout_metrics() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("l1", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("l2", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("r1", StatusBarAlignment::Right, 0));
        bar.set_visibility("l2", false);
        let layout = bar.compute_layout();
        assert_eq!(layout.left_count, 1);
        assert_eq!(layout.right_count, 1);
        assert_eq!(layout.total_visible, 2);
        assert_eq!(layout.total_hidden, 1);
    }

    #[test]
    fn set_group_visibility_toggles() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("c", StatusBarAlignment::Right, 0));
        let mut group = StatusBarGroup::new("ab");
        group.add("a");
        group.add("b");
        bar.set_group_visibility(&group, false);
        assert!(!bar.get_entry("a").unwrap().visible);
        assert!(!bar.get_entry("b").unwrap().visible);
        assert!(bar.get_entry("c").unwrap().visible);
    }

    #[test]
    fn collect_and_clear_tooltips() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("t1", "text", StatusBarAlignment::Left)
                .tooltip("tip1")
                .build(),
        );
        bar.add_entry(make_entry("t2", StatusBarAlignment::Left, 0));
        let tips = bar.collect_tooltips();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0], ("t1", "tip1"));
        bar.clear_tooltips();
        assert!(bar.collect_tooltips().is_empty());
    }

    #[test]
    fn toggle_all_visibility_works() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 1));
        assert_eq!(bar.visible_count(), 2);
        bar.toggle_all_visibility();
        assert_eq!(bar.visible_count(), 0);
        bar.toggle_all_visibility();
        assert_eq!(bar.visible_count(), 2);
    }

    #[test]
    fn compute_layout_empty_bar() {
        let bar = StatusBar::new();
        let layout = bar.compute_layout();
        assert_eq!(layout.left_count, 0);
        assert_eq!(layout.right_count, 0);
        assert_eq!(layout.total_visible, 0);
        assert_eq!(layout.total_hidden, 0);
        assert_eq!(layout.left_text_width, 0);
        assert_eq!(layout.right_text_width, 0);
    }

    #[test]
    fn priority_tier_ordering() {
        assert!(StatusBarPriorityTier::Essential < StatusBarPriorityTier::Standard);
        assert!(StatusBarPriorityTier::Standard < StatusBarPriorityTier::Optional);
    }

    #[test]
    fn priority_tier_display() {
        assert_eq!(format!("{}", StatusBarPriorityTier::Essential), "Essential");
        assert_eq!(format!("{}", StatusBarPriorityTier::Standard), "Standard");
        assert_eq!(format!("{}", StatusBarPriorityTier::Optional), "Optional");
    }

    #[test]
    fn tooltip_render_basic() {
        let tip = StatusBarTooltip::new("git", "Current Branch: main");
        assert_eq!(tip.render(), "Current Branch: main");
    }

    #[test]
    fn tooltip_render_with_description_and_shortcut() {
        let tip = StatusBarTooltip::new("git", "Branch")
            .with_description("Currently on main")
            .with_shortcut("Ctrl+Shift+G");
        let rendered = tip.render();
        assert!(rendered.contains("Branch"));
        assert!(rendered.contains("Currently on main"));
        assert!(rendered.contains("(Ctrl+Shift+G)"));
    }

    #[test]
    fn tooltip_display() {
        let tip = StatusBarTooltip::new("id", "Title");
        assert_eq!(format!("{tip}"), "Title");
    }

    #[test]
    fn render_with_separator_works() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 10));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("c", StatusBarAlignment::Right, 5));
        bar.update_text("a", "A");
        bar.update_text("b", "B");
        bar.update_text("c", "C");
        let (left, right) = bar.render_with_separator(" | ");
        assert_eq!(left, "B | A");
        assert_eq!(right, "C");
    }

    #[test]
    fn apply_visibility_rule_always() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        bar.set_visibility("x", false);
        assert!(bar.apply_visibility_rule("x", StatusBarVisibility::Always));
        assert!(bar.get_entry("x").unwrap().visible);
    }

    #[test]
    fn apply_visibility_rule_hidden() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        bar.apply_visibility_rule("x", StatusBarVisibility::Hidden);
        assert!(!bar.get_entry("x").unwrap().visible);
    }

    #[test]
    fn apply_visibility_rule_when_non_empty() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("x", StatusBarAlignment::Left, 0));
        bar.update_text("x", "");
        bar.apply_visibility_rule("x", StatusBarVisibility::WhenNonEmpty);
        assert!(!bar.get_entry("x").unwrap().visible);
        bar.update_text("x", "content");
        bar.apply_visibility_rule("x", StatusBarVisibility::WhenNonEmpty);
        assert!(bar.get_entry("x").unwrap().visible);
    }

    #[test]
    fn render_tooltips_collects_visible() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("a", "text", StatusBarAlignment::Left)
                .tooltip("Tooltip A")
                .build(),
        );
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 0));
        let tooltips = bar.render_tooltips();
        assert_eq!(tooltips.len(), 1);
        assert_eq!(tooltips[0].entry_id, "a");
        assert_eq!(tooltips[0].title, "Tooltip A");
    }

    #[test]
    fn entries_by_tier_ordering() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("optional", StatusBarAlignment::Left, 100));
        bar.add_entry(make_entry("essential", StatusBarAlignment::Left, -10));
        bar.add_entry(make_entry("standard", StatusBarAlignment::Left, 25));
        let tiered = bar.entries_by_tier();
        assert_eq!(tiered[0].1, StatusBarPriorityTier::Essential);
        assert_eq!(tiered[1].1, StatusBarPriorityTier::Standard);
        assert_eq!(tiered[2].1, StatusBarPriorityTier::Optional);
    }

    #[test]
    fn hide_optional_show_essential() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("e", StatusBarAlignment::Left, -5));
        bar.add_entry(make_entry("s", StatusBarAlignment::Left, 25));
        bar.add_entry(make_entry("o", StatusBarAlignment::Left, 100));
        bar.set_visibility("e", false);
        bar.hide_optional_entries();
        assert!(!bar.get_entry("o").unwrap().visible);
        assert!(bar.get_entry("s").unwrap().visible);
        bar.show_essential_entries();
        assert!(bar.get_entry("e").unwrap().visible);
    }

    #[test]
    fn apply_visibility_rule_missing_entry() {
        let mut bar = StatusBar::new();
        assert!(!bar.apply_visibility_rule("nope", StatusBarVisibility::Always));
    }

    #[test]
    fn entry_query_helpers() {
        let entry = StatusBarEntry::builder("git", "main", StatusBarAlignment::Left)
            .tooltip("branch info")
            .build();
        assert!(entry.is_visible());
        assert!(entry.has_tooltip());
        assert!(!entry.has_command());
        assert!(entry.matches_filter("GIT"));
        assert!(entry.matches_filter("mai"));
        assert!(!entry.matches_filter("zzz"));
    }

    #[test]
    fn statusbar_is_empty_and_iter() {
        let mut bar = StatusBar::new();
        assert!(bar.is_empty());
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 1));
        assert!(!bar.is_empty());
        let ids: Vec<&str> = bar.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        let ids2: Vec<&str> = (&bar).into_iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids2, vec!["a", "b"]);
    }

    #[test]
    fn visible_entries_and_find_by_id() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("v", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("h", StatusBarAlignment::Left, 1));
        bar.set_visibility("h", false);
        assert_eq!(bar.visible_entries().len(), 1);
        assert_eq!(bar.visible_entries()[0].id, "v");
        assert!(bar.find_by_id("v").is_some());
        assert!(bar.find_by_id("missing").is_none());
    }

    #[test]
    fn snapshot_diff_detects_changes() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 1));
        let snap1 = bar.snapshot();
        bar.update_text("a", "changed");
        bar.remove_entry("b");
        bar.add_entry(make_entry("c", StatusBarAlignment::Left, 2));
        let snap2 = bar.snapshot();
        let diff = snap1.diff(&snap2);
        assert!(diff.iter().any(|d| d.contains("changed text: a")));
        assert!(diff.iter().any(|d| d.contains("removed: b")));
        assert!(diff.iter().any(|d| d.contains("added: c")));
        assert_eq!(format!("{}", snap1), "StatusBarSnapshot(2 entries)");
    }

    #[test]
    fn group_merge_and_entry_count() {
        let mut g1 = StatusBarGroup::new("g1");
        g1.add("a");
        g1.add("b");
        let mut g2 = StatusBarGroup::new("g2");
        g2.add("b");
        g2.add("c");
        g1.merge(&g2);
        assert_eq!(g1.entry_count(), 3);
        assert!(g1.contains("c"));
    }

    #[test]
    fn layout_total_width_and_accessors() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("left", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("right", StatusBarAlignment::Right, 0));
        bar.update_text("left", "hello");
        bar.update_text("right", "world!");
        let layout = bar.compute_layout();
        assert_eq!(layout.total_width(), 11);
        assert_eq!(layout.left_count(), 1);
        assert_eq!(layout.right_count(), 1);
    }

    #[test]
    fn priority_tier_helpers() {
        assert!(StatusBarPriorityTier::Essential.is_high());
        assert!(!StatusBarPriorityTier::Standard.is_high());
        assert_eq!(StatusBarPriorityTier::Optional.label(), "optional");
    }

    #[test]
    fn tooltip_is_empty_and_word_count() {
        let empty = StatusBarTooltip::new("id", "");
        assert!(empty.is_empty());
        assert_eq!(empty.word_count(), 0);
        let tip = StatusBarTooltip::new("id", "Hello World")
            .with_description("A longer description here");
        assert!(!tip.is_empty());
        assert_eq!(tip.word_count(), 6);
    }

    #[test]
    fn visibility_is_shown_and_display() {
        assert!(StatusBarVisibility::Always.is_shown());
        assert!(!StatusBarVisibility::Hidden.is_shown());
        assert!(!StatusBarVisibility::WhenNonEmpty.is_shown());
        assert_eq!(format!("{}", StatusBarVisibility::Always), "Always");
        assert_eq!(format!("{}", StatusBarVisibility::Hidden), "Hidden");
        assert_eq!(format!("{}", StatusBarVisibility::WhenNonEmpty), "WhenNonEmpty");
    }

    #[test]
    fn summary_statistics() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("a", "text", StatusBarAlignment::Left)
                .tooltip("tip")
                .command("cmd")
                .build(),
        );
        bar.add_entry(make_entry("b", StatusBarAlignment::Right, 1));
        bar.set_visibility("b", false);
        let s = bar.summary();
        assert_eq!(s.total, 2);
        assert_eq!(s.visible, 1);
        assert_eq!(s.hidden, 1);
        assert_eq!(s.left, 1);
        assert_eq!(s.right, 1);
        assert_eq!(s.with_tooltip, 1);
        assert_eq!(s.with_command, 1);
        let display = format!("{s}");
        assert!(display.contains("total=2"));
        assert!(display.contains("commands=1"));
    }

    // --- Animation, click routing, space allocation, overflow tests ---

    #[test]
    fn animation_state_properties() {
        assert!(!AnimationState::Idle.is_animating());
        assert!(AnimationState::FadingIn.is_animating());
        assert!(AnimationState::FadingOut.is_animating());
        assert!(AnimationState::Pulsing.is_animating());
        assert!(AnimationState::Spinning.is_animating());
        assert!(AnimationState::FadingIn.is_transition());
        assert!(AnimationState::FadingOut.is_transition());
        assert!(!AnimationState::Pulsing.is_transition());
        assert!(!AnimationState::Idle.is_transition());
        assert_eq!(format!("{}", AnimationState::Spinning), "Spinning");
    }

    #[test]
    fn click_action_routing() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("git", "main", StatusBarAlignment::Left)
                .command("git.checkout")
                .build(),
        );
        bar.add_entry(make_entry("info", StatusBarAlignment::Left, 0));
        assert_eq!(
            bar.route_click("git"),
            ClickAction::RunCommand("git.checkout".to_string())
        );
        assert_eq!(bar.route_click("info"), ClickAction::None);
        assert_eq!(bar.route_click("missing"), ClickAction::None);
    }

    #[test]
    fn click_action_helpers_and_display() {
        let cmd = ClickAction::RunCommand("editor.save".to_string());
        assert!(cmd.is_actionable());
        assert_eq!(cmd.command_name(), Some("editor.save"));
        assert_eq!(format!("{cmd}"), "command:editor.save");

        let url = ClickAction::OpenUrl("https://example.com".to_string());
        assert!(url.is_actionable());
        assert!(url.command_name().is_none());
        assert_eq!(format!("{url}"), "url:https://example.com");

        let menu = ClickAction::ShowMenu(vec!["a".into(), "b".into()]);
        assert!(menu.is_actionable());
        assert_eq!(format!("{menu}"), "menu[2]");

        let none = ClickAction::None;
        assert!(!none.is_actionable());
        assert_eq!(format!("{none}"), "none");
    }

    #[test]
    fn space_allocation_fits_all() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("a", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("b", StatusBarAlignment::Left, 2));
        bar.update_text("a", "Hi");
        bar.update_text("b", "Lo");
        // "Hi" (2) + sep (1) + "Lo" (2) = 5
        let alloc = bar.allocate_space(StatusBarAlignment::Left, 10, 1);
        assert_eq!(alloc.displayed, vec!["a", "b"]);
        assert!(alloc.overflowed.is_empty());
        assert_eq!(alloc.consumed_width, 5);
        assert_eq!(alloc.remaining_width, 5);
    }

    #[test]
    fn space_allocation_overflow() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("high", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("low", StatusBarAlignment::Left, 10));
        bar.update_text("high", "AAAA");
        bar.update_text("low", "BBBB");
        // "AAAA"(4) + sep(1) + "BBBB"(4) = 9, only 6 available
        let alloc = bar.allocate_space(StatusBarAlignment::Left, 6, 1);
        assert_eq!(alloc.displayed, vec!["high"]);
        assert_eq!(alloc.overflowed, vec!["low"]);
        assert_eq!(alloc.consumed_width, 4);
        assert_eq!(alloc.remaining_width, 2);
    }

    #[test]
    fn generate_tooltip_with_command() {
        let mut bar = StatusBar::new();
        bar.add_entry(
            StatusBarEntry::builder("git", "main", StatusBarAlignment::Left)
                .tooltip("Current branch")
                .command("git.checkout")
                .priority(-5)
                .build(),
        );
        let tip = bar.generate_tooltip("git").unwrap();
        assert_eq!(tip.entry_id, "git");
        assert_eq!(tip.title, "Current branch");
        assert!(tip.description.as_ref().unwrap().contains("Essential"));
        assert_eq!(tip.shortcut.as_deref(), Some("git.checkout"));
        // Missing entry returns None
        assert!(bar.generate_tooltip("missing").is_none());
    }

    #[test]
    fn overflow_entries_across_sides() {
        let mut bar = StatusBar::new();
        bar.add_entry(make_entry("l1", StatusBarAlignment::Left, 0));
        bar.add_entry(make_entry("l2", StatusBarAlignment::Left, 1));
        bar.add_entry(make_entry("r1", StatusBarAlignment::Right, 0));
        bar.update_text("l1", "AAAA");
        bar.update_text("l2", "BBBB");
        bar.update_text("r1", "CC");
        // total_width=10 => half=5 for each side
        // Left: "AAAA"(4) fits, "BBBB" needs 4+1=5 more (total 9 > 5) => overflow
        // Right: "CC"(2) fits
        let overflow = bar.overflow_entries(10, 1);
        assert_eq!(overflow, vec!["l2"]);
    }

    // ---- Tests for StatusBarAlignmentPriority ----

    #[test]
    fn test_alignment_priority_left_before_right() {
        let left = StatusBarAlignmentPriority::new(StatusBarAlignment::Left, 5, "a");
        let right = StatusBarAlignmentPriority::new(StatusBarAlignment::Right, 1, "b");
        assert!(left < right, "Left items should sort before Right items");
    }

    #[test]
    fn test_alignment_priority_sort_by_priority() {
        let low = StatusBarAlignmentPriority::new(StatusBarAlignment::Left, 1, "low");
        let high = StatusBarAlignmentPriority::new(StatusBarAlignment::Left, 10, "high");
        assert!(low < high, "Lower priority value should sort first");
    }

    #[test]
    fn test_alignment_priority_sorted_items() {
        let items = vec![
            StatusBarAlignmentPriority::new(StatusBarAlignment::Right, 2, "r2"),
            StatusBarAlignmentPriority::new(StatusBarAlignment::Left, 5, "l5"),
            StatusBarAlignmentPriority::new(StatusBarAlignment::Left, 1, "l1"),
            StatusBarAlignmentPriority::new(StatusBarAlignment::Right, 0, "r0"),
        ];
        let sorted = StatusBarAlignmentPriority::sorted_items(&items);
        assert_eq!(sorted[0].item_id, "l1");
        assert_eq!(sorted[1].item_id, "l5");
        assert_eq!(sorted[2].item_id, "r0");
        assert_eq!(sorted[3].item_id, "r2");
    }

    // ---- Tests for StatusBarItemGroup ----

    #[test]
    fn test_item_group_add_and_contains() {
        let mut group = StatusBarItemGroup::new("git-info");
        group.add_item("branch");
        group.add_item("sync");
        assert!(group.contains("branch"));
        assert!(group.contains("sync"));
        assert!(!group.contains("stash"));
        assert_eq!(group.item_count(), 2);
    }

    #[test]
    fn test_item_group_no_duplicates() {
        let mut group = StatusBarItemGroup::new("g1");
        group.add_item("x");
        group.add_item("x");
        assert_eq!(group.item_count(), 1);
    }

    #[test]
    fn test_item_group_remove_item() {
        let mut group = StatusBarItemGroup::new("g1");
        group.add_item("a");
        group.add_item("b");
        assert!(group.remove_item("a"));
        assert!(!group.contains("a"));
        assert!(!group.remove_item("nonexistent"));
        assert_eq!(group.item_count(), 1);
    }

    #[test]
    fn test_item_group_toggle_collapse() {
        let mut group = StatusBarItemGroup::new("g1");
        assert!(!group.collapsed);
        group.toggle_collapse();
        assert!(group.collapsed);
        group.toggle_collapse();
        assert!(!group.collapsed);
    }

    #[test]
    fn test_item_group_display() {
        let mut group = StatusBarItemGroup::new("editors");
        group.add_item("tab1");
        group.add_item("tab2");
        let display = format!("{}", group);
        assert_eq!(display, "Group 'editors' (expanded, 2 items)");
        group.toggle_collapse();
        let display = format!("{}", group);
        assert_eq!(display, "Group 'editors' (collapsed, 2 items)");
    }

    // ---- Tests for StatusBarTooltipBuilder ----

    #[test]
    fn test_tooltip_builder_title_only() {
        let tooltip = StatusBarTooltipBuilder::new("Git Branch")
            .build("git-branch");
        assert_eq!(tooltip.title, "Git Branch");
        assert_eq!(tooltip.entry_id, "git-branch");
        assert!(tooltip.description.is_none());
    }

    #[test]
    fn test_tooltip_builder_with_lines_and_links() {
        let tooltip = StatusBarTooltipBuilder::new("Encoding")
            .add_line("Current: UTF-8")
            .add_line("Click to change")
            .add_link("Docs", "https://example.com/encoding")
            .build("encoding");
        assert_eq!(tooltip.title, "Encoding");
        let desc = tooltip.description.unwrap();
        assert!(desc.contains("Current: UTF-8"));
        assert!(desc.contains("Click to change"));
        assert!(desc.contains("[Docs](https://example.com/encoding)"));
    }

    // ---- Tests for StatusBarItemToggle ----

    #[test]
    fn test_item_toggle_initial_state() {
        let toggle = StatusBarItemToggle::new("line-col");
        assert!(toggle.is_visible());
        assert_eq!(toggle.toggle_count, 0);
        assert_eq!(toggle.item_id, "line-col");
    }

    #[test]
    fn test_item_toggle_toggle_and_count() {
        let mut toggle = StatusBarItemToggle::new("indent");
        toggle.toggle();
        assert!(!toggle.is_visible());
        assert_eq!(toggle.toggle_count, 1);
        toggle.toggle();
        assert!(toggle.is_visible());
        assert_eq!(toggle.toggle_count, 2);
    }

    #[test]
    fn test_item_toggle_reset() {
        let mut toggle = StatusBarItemToggle::new("lang");
        toggle.toggle();
        toggle.toggle();
        toggle.toggle();
        assert!(!toggle.is_visible());
        assert_eq!(toggle.toggle_count, 3);
        toggle.reset();
        assert!(toggle.is_visible());
        assert_eq!(toggle.toggle_count, 0);
    }
    #[test]
    fn animation_tick_basic() {
        let mut anim = StatusBarAnimationTick::spinner(100);
        assert_eq!(anim.frame_count(), 10);
        assert!(!anim.is_running());
        anim.start();
        assert!(anim.is_running());
        assert_eq!(anim.current_frame_str(), "⠋");
        assert!(!anim.tick(50));
        assert!(anim.tick(60));
        assert_eq!(anim.current_index(), 1);
    }

    #[test]
    fn animation_tick_wrap_around() {
        let mut anim = StatusBarAnimationTick::dots(10);
        anim.start();
        for _ in 0..4 {
            anim.tick(10);
        }
        assert_eq!(anim.current_index(), 0);
    }

    #[test]
    fn animation_tick_stop_reset() {
        let mut anim = StatusBarAnimationTick::spinner(50);
        anim.start();
        anim.tick(50);
        assert_eq!(anim.current_index(), 1);
        anim.stop();
        assert!(!anim.is_running());
        assert!(!anim.tick(100));
        anim.reset();
        assert_eq!(anim.current_index(), 0);
    }

    #[test]
    fn animation_set_interval() {
        let mut anim = StatusBarAnimationTick::spinner(100);
        assert_eq!(anim.interval_ms(), 100);
        anim.set_interval_ms(200);
        assert_eq!(anim.interval_ms(), 200);
    }

    #[test]
    fn separator_style_as_str() {
        assert_eq!(SeparatorStyle::Pipe.as_str(), " | ");
        assert_eq!(SeparatorStyle::Dot.as_str(), " · ");
        assert_eq!(SeparatorStyle::Dash.as_str(), " - ");
        assert_eq!(SeparatorStyle::Space.as_str(), "  ");
        assert_eq!(SeparatorStyle::Custom('•').as_str(), " • ");
    }

    #[test]
    fn separator_renderer_join() {
        let renderer = StatusBarSeparatorRenderer::new(SeparatorStyle::Pipe);
        let result = renderer.join_texts(&["git", "main"], StatusBarAlignment::Left);
        assert_eq!(result, "git | main");
    }

    #[test]
    fn separator_renderer_different_sides() {
        let renderer = StatusBarSeparatorRenderer::with_different_sides(
            SeparatorStyle::Pipe,
            SeparatorStyle::Dot,
        );
        assert_eq!(renderer.left_style(), SeparatorStyle::Pipe);
        assert_eq!(renderer.right_style(), SeparatorStyle::Dot);
    }

    #[test]
    fn separator_renderer_full_bar() {
        let renderer = StatusBarSeparatorRenderer::default();
        let bar = renderer.render_bar(&["git"], &["Ln 1"], 40);
        assert_eq!(bar.len(), 40);
        assert!(bar.starts_with("git"));
        assert!(bar.ends_with("Ln 1"));
    }

    #[test]
    fn priority_manager_register_order() {
        let mut mgr = StatusBarPriorityManager::new();
        mgr.register("git", 100, StatusBarAlignment::Left);
        mgr.register("encoding", 50, StatusBarAlignment::Right);
        mgr.register("lang", 200, StatusBarAlignment::Left);
        let left_order = mgr.ordered_ids(StatusBarAlignment::Left);
        assert_eq!(left_order, vec!["lang", "git"]);
    }

    #[test]
    fn priority_manager_unregister() {
        let mut mgr = StatusBarPriorityManager::new();
        mgr.register("a", 10, StatusBarAlignment::Left);
        assert!(mgr.unregister("a"));
        assert!(!mgr.unregister("a"));
        assert_eq!(mgr.item_count(), 0);
    }

    #[test]
    fn priority_manager_update_existing() {
        let mut mgr = StatusBarPriorityManager::new();
        mgr.register("x", 10, StatusBarAlignment::Left);
        mgr.register("x", 99, StatusBarAlignment::Right);
        assert_eq!(mgr.item_count(), 1);
        assert_eq!(mgr.get_priority("x"), Some(99));
    }

    #[test]
    fn priority_manager_set_priority() {
        let mut mgr = StatusBarPriorityManager::new();
        mgr.register("a", 10, StatusBarAlignment::Left);
        assert!(mgr.set_priority("a", 50));
        assert_eq!(mgr.get_priority("a"), Some(50));
        assert!(!mgr.set_priority("none", 0));
    }

    #[test]
    fn priority_manager_highest() {
        let mut mgr = StatusBarPriorityManager::new();
        mgr.register("a", 10, StatusBarAlignment::Left);
        mgr.register("b", 100, StatusBarAlignment::Left);
        mgr.register("c", 50, StatusBarAlignment::Left);
        let h = mgr.highest_priority(StatusBarAlignment::Left).unwrap();
        assert_eq!(h.entry_id, "b");
        assert!(mgr.highest_priority(StatusBarAlignment::Right).is_none());
    }

    #[test]
    fn bg_color_manager_defaults() {
        let mgr = StatusBarBackgroundColorManager::default();
        assert_eq!(mgr.default_color(), "#007acc");
        assert_eq!(mgr.error_color(), "#e51400");
        assert_eq!(mgr.warning_color(), "#c8a000");
    }

    #[test]
    fn bg_color_manager_overrides() {
        let mut mgr = StatusBarBackgroundColorManager::new("#000");
        mgr.set_override("git", "#ff0000");
        assert_eq!(mgr.resolve_color("git"), "#ff0000");
        assert_eq!(mgr.resolve_color("other"), "#000");
        assert_eq!(mgr.override_count(), 1);
        assert!(mgr.remove_override("git"));
        assert_eq!(mgr.resolve_color("git"), "#000");
    }

    #[test]
    fn bg_color_manager_status_color() {
        let mgr = StatusBarBackgroundColorManager::default();
        assert_eq!(mgr.resolve_status_color(true, false), "#e51400");
        assert_eq!(mgr.resolve_status_color(false, true), "#c8a000");
        assert_eq!(mgr.resolve_status_color(false, false), "#007acc");
        assert_eq!(mgr.resolve_status_color(true, true), "#e51400");
    }

    #[test]
    fn bg_color_manager_set_colors() {
        let mut mgr = StatusBarBackgroundColorManager::new("#000");
        mgr.set_default_color("#111");
        mgr.set_error_color("#222");
        mgr.set_warning_color("#333");
        assert_eq!(mgr.default_color(), "#111");
        assert_eq!(mgr.error_color(), "#222");
        assert_eq!(mgr.warning_color(), "#333");
    }

    #[test]
    fn bg_color_manager_clear_overrides() {
        let mut mgr = StatusBarBackgroundColorManager::new("#000");
        mgr.set_override("a", "#aaa");
        mgr.set_override("b", "#bbb");
        mgr.clear_overrides();
        assert_eq!(mgr.override_count(), 0);
    }


    // -- statusbar additional tests -------------------------------------------

    #[test]
    fn x_statusbar_panel_state_new() {
        let p = XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XStatusbarLayoutRegion::Sidebar);
    }

    #[test]
    fn x_statusbar_panel_area() {
        let p = XStatusbarPanelState::new(XStatusbarLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_statusbar_panel_toggle() {
        let mut p = XStatusbarPanelState::new(XStatusbarLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_statusbar_panel_resize() {
        let mut p = XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_statusbar_panel_is_narrow() {
        let mut p = XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_statusbar_total_visible_area_basic() {
        let panels = vec![
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "a"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_statusbar_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_statusbar_total_visible_area_hidden() {
        let mut panels = vec![
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "a"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_statusbar_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_statusbar_count_in_region_basic() {
        let panels = vec![
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "a"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "b"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_statusbar_count_in_region(&panels, XStatusbarLayoutRegion::Sidebar), 2);
        assert_eq!(x_statusbar_count_in_region(&panels, XStatusbarLayoutRegion::Editor), 1);
        assert_eq!(x_statusbar_count_in_region(&panels, XStatusbarLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_statusbar_widest_panel_basic() {
        let mut panels = vec![
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "narrow"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_statusbar_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_statusbar_collapse_region_basic() {
        let mut panels = vec![
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "a"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Sidebar, "b"),
            XStatusbarPanelState::new(XStatusbarLayoutRegion::Editor, "c"),
        ];
        x_statusbar_collapse_region(&mut panels, XStatusbarLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_statusbar_layout_constraint_clamp() {
        let lc = XStatusbarLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_statusbar_layout_constraint_satisfied() {
        let lc = XStatusbarLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_statusbar_widest_panel_empty() {
        let panels: Vec<XStatusbarPanelState> = vec![];
        assert!(x_statusbar_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_statusbar_layout_region_eq() {
        assert_eq!(XStatusbarLayoutRegion::Sidebar, XStatusbarLayoutRegion::Sidebar);
        assert_ne!(XStatusbarLayoutRegion::Sidebar, XStatusbarLayoutRegion::Panel);
    }


    // -- statusbar extended domain tests ----------------------------------------

    #[test]
    fn y_statusbar_enum_index() {
        assert_eq!(YStatusbarStatusbarAlignment::Left.index(), 0);
        assert_eq!(YStatusbarStatusbarAlignment::Right.index(), 1);
        assert_eq!(YStatusbarStatusbarAlignment::Center.index(), 2);
        assert_eq!(YStatusbarStatusbarAlignment::Hidden.index(), 3);
    }

    #[test]
    fn y_statusbar_enum_label() {
        assert_eq!(YStatusbarStatusbarAlignment::Left.label(), "Left");
        assert_eq!(YStatusbarStatusbarAlignment::Right.label(), "Right");
        assert_eq!(YStatusbarStatusbarAlignment::Center.label(), "Center");
        assert_eq!(YStatusbarStatusbarAlignment::Hidden.label(), "Hidden");
    }

    #[test]
    fn y_statusbar_enum_all() {
        let all = YStatusbarStatusbarAlignment::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_statusbar_enum_is_default() {
        assert!(YStatusbarStatusbarAlignment::Left.is_default());
        assert!(!YStatusbarStatusbarAlignment::Hidden.is_default());
    }

    #[test]
    fn y_statusbar_enum_display() {
        assert_eq!(format!("{}", YStatusbarStatusbarAlignment::Left), "Left");
    }

    #[test]
    fn y_statusbar_struct_new() {
        let s = YStatusbarStatusbarItem::new();
        let _ = s.summary();
    }

    #[test]
    fn y_statusbar_fingerprint_deterministic() {
        let h1 = y_statusbar_fingerprint("hello");
        let h2 = y_statusbar_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_statusbar_fingerprint("a"), y_statusbar_fingerprint("b"));
    }

    #[test]
    fn y_statusbar_truncate_short() {
        assert_eq!(y_statusbar_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_statusbar_truncate_long() {
        let r = y_statusbar_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_statusbar_normalize_key_basic() {
        assert_eq!(y_statusbar_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_statusbar_split_path_basic() {
        let parts = y_statusbar_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_statusbar_count_occurrences_basic() {
        assert_eq!(y_statusbar_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_statusbar_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_statusbar_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_statusbar_in_range_basic() {
        assert!(y_statusbar_in_range(5, 1, 10));
        assert!(y_statusbar_in_range(1, 1, 10));
        assert!(y_statusbar_in_range(10, 1, 10));
        assert!(!y_statusbar_in_range(0, 1, 10));
        assert!(!y_statusbar_in_range(11, 1, 10));
    }

    #[test]
    fn y_statusbar_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_statusbar_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_statusbar_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_statusbar_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- statusbar Z-extended tests -----------------------------------------------

    #[test]
    fn z_statusbar_priority_weight() {
        assert_eq!(ZStatusbarPriority::Idle.weight(), 0);
        assert_eq!(ZStatusbarPriority::Normal.weight(), 2);
        assert_eq!(ZStatusbarPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_statusbar_priority_label() {
        assert_eq!(ZStatusbarPriority::Low.label(), "low");
        assert_eq!(ZStatusbarPriority::High.label(), "high");
    }

    #[test]
    fn z_statusbar_priority_is_elevated() {
        assert!(!ZStatusbarPriority::Normal.is_elevated());
        assert!(ZStatusbarPriority::High.is_elevated());
        assert!(ZStatusbarPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_statusbar_priority_display() {
        assert_eq!(format!("{}", ZStatusbarPriority::Idle), "idle");
    }

    #[test]
    fn z_statusbar_priority_all_asc() {
        let all = ZStatusbarPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZStatusbarPriority::Idle);
        assert_eq!(all[4], ZStatusbarPriority::Realtime);
    }

    #[test]
    fn z_statusbar_struct_new() {
        let s = ZStatusbarStatusbarAnimation::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_statusbar_struct_toggled_clone() {
        let s = ZStatusbarStatusbarAnimation::new();
        let t = s.toggled_clone();
        assert_ne!(s.looping, t.looping);
    }

    #[test]
    fn z_statusbar_rolling_hash_deterministic() {
        let h1 = z_statusbar_rolling_hash(b"test");
        let h2 = z_statusbar_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_statusbar_rolling_hash(b"a"), z_statusbar_rolling_hash(b"b"));
    }

    #[test]
    fn z_statusbar_pad_to_basic() {
        assert_eq!(z_statusbar_pad_to("hi", 5), "hi   ");
        assert_eq!(z_statusbar_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_statusbar_is_identifier_basic() {
        assert!(z_statusbar_is_identifier("foo_bar"));
        assert!(z_statusbar_is_identifier("abc123"));
        assert!(!z_statusbar_is_identifier(""));
        assert!(!z_statusbar_is_identifier("has space"));
    }

    #[test]
    fn z_statusbar_levenshtein_basic() {
        assert_eq!(z_statusbar_levenshtein("", ""), 0);
        assert_eq!(z_statusbar_levenshtein("abc", "abc"), 0);
        assert_eq!(z_statusbar_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_statusbar_unique_words_basic() {
        let w = z_statusbar_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_statusbar_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_statusbar_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_statusbar_common_prefix_basic() {
        assert_eq!(z_statusbar_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_statusbar_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_statusbar_struct_clear() {
        let mut s = ZStatusbarStatusbarAnimation::new();
        s.frames.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_statusbar_rolling_hash_empty() {
        let h = z_statusbar_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_68_push_and_len() {
        let mut rb = super::XbRingBuffer68::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_68_overwrite() {
        let mut rb = super::XbRingBuffer68::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_68_get_out_of_bounds() {
        let rb = super::XbRingBuffer68::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_68_drain_all() {
        let mut rb = super::XbRingBuffer68::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_68_peek_front_back() {
        let mut rb = super::XbRingBuffer68::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_68_clear() {
        let mut rb = super::XbRingBuffer68::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_68_capacity() {
        let rb = super::XbRingBuffer68::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_68_basic() {
        let h = super::xb_fnv1a_68(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_68(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_68_different_inputs() {
        let h1 = super::xb_fnv1a_68(b"abc");
        let h2 = super::xb_fnv1a_68(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_68_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_68(&data);
        let dec = super::xb_rle_decode_68(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_68_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_68(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_68(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_68_values() {
        assert!((super::xb_clamp_68(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_68(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_68(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_68_values() {
        assert!((super::xb_lerp_68(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_68(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_68(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_68_wrap_around_twice() {
        let mut rb = super::XbRingBuffer68::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 164 ----

    #[test]
    fn xc_164_pool_new_empty() {
        let pool: super::Xc164Pool<i32> = super::Xc164Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_164_pool_release_acquire() {
        let mut pool = super::Xc164Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_164_pool_acquire_empty() {
        let mut pool: super::Xc164Pool<i32> = super::Xc164Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_164_pool_full() {
        let mut pool = super::Xc164Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_164_pool_drain() {
        let mut pool = super::Xc164Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_164_pool_stats() {
        let mut pool = super::Xc164Pool::new(8);
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
    fn xc_164_pool_clear() {
        let mut pool = super::Xc164Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_164_pool_shrink() {
        let mut pool = super::Xc164Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_164_pool_default() {
        let pool: super::Xc164Pool<String> = super::Xc164Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_164_pool_extend() {
        let mut pool = super::Xc164Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_164_pool_retain() {
        let mut pool = super::Xc164Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_164_scheduler_round_robin() {
        let mut sched = super::Xc164Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_164_scheduler_empty() {
        let mut sched = super::Xc164Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_164_scheduler_reset() {
        let mut sched = super::Xc164Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_164_scheduler_add_remove() {
        let mut sched = super::Xc164Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_164_scheduler_targets() {
        let sched = super::Xc164Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_164_hash_empty() {
        assert_eq!(super::xc_164_hash(b""), 5381);
    }

    #[test]
    fn xc_164_hash_data() {
        let h = super::xc_164_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_164_hash(b"hello"), h);
    }

    #[test]
    fn xc_164_reverse_str() {
        assert_eq!(super::xc_164_reverse("abc"), "cba");
        assert_eq!(super::xc_164_reverse(""), "");
    }

}
