//! Editor group and tab management service.
//!
//! Equivalent to VS Code's `vs/workbench/services/editor/common/editorService.ts`.
//! Manages editor instances within groups (tab strips) and exposes events for
//! active-editor changes.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

// ---------------------------------------------------------------------------
// EditorInput
// ---------------------------------------------------------------------------

/// Describes what is open in an editor tab.
#[derive(Debug, Clone)]
pub struct EditorInput {
    /// The resource this editor is showing.
    pub uri: VsUri,
    /// Display name for the tab.
    pub name: String,
    /// Whether the editor has unsaved changes.
    pub is_dirty: bool,
    /// Whether the editor content is read-only.
    pub is_readonly: bool,
    /// Optional language identifier (e.g. `"rust"`, `"python"`).
    pub language_id: Option<String>,
}

impl PartialEq for EditorInput {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
            && self.name == other.name
            && self.is_dirty == other.is_dirty
            && self.is_readonly == other.is_readonly
            && self.language_id == other.language_id
    }
}

// ---------------------------------------------------------------------------
// EditorGroup
// ---------------------------------------------------------------------------

/// A group of editor tabs (a single tab strip / split pane).
#[derive(Debug)]
pub struct EditorGroup {
    /// Unique group identifier.
    pub id: u32,
    editors: Vec<EditorInput>,
    active_index: Option<usize>,
}

impl EditorGroup {
    /// Create a new empty group with the given id.
    fn new(id: u32) -> Self {
        Self {
            id,
            editors: Vec::new(),
            active_index: None,
        }
    }

    /// Open an editor input in this group.
    ///
    /// If a tab with the same URI is already open, that tab is activated
    /// instead of opening a duplicate.
    pub fn open(&mut self, input: EditorInput) {
        if let Some(idx) = self.find_by_uri(&input.uri) {
            self.active_index = Some(idx);
            return;
        }
        self.editors.push(input);
        self.active_index = Some(self.editors.len() - 1);
    }

    /// Close the editor at `index`.
    ///
    /// If the closed tab was active, the next tab is activated (or the
    /// previous one if the closed tab was last).
    pub fn close(&mut self, index: usize) {
        if index >= self.editors.len() {
            return;
        }
        self.editors.remove(index);

        if self.editors.is_empty() {
            self.active_index = None;
        } else if let Some(active) = self.active_index {
            if index == active {
                // Activate next, or previous if we closed the last tab.
                self.active_index = Some(index.min(self.editors.len() - 1));
            } else if index < active {
                self.active_index = Some(active - 1);
            }
        }
    }

    /// Set the active tab by index.
    pub fn set_active(&mut self, index: usize) {
        if index < self.editors.len() {
            self.active_index = Some(index);
        }
    }

    /// Return the currently active editor, if any.
    pub fn active_editor(&self) -> Option<&EditorInput> {
        self.active_index.and_then(|i| self.editors.get(i))
    }

    /// Return a slice of all editors in this group.
    pub fn get_editors(&self) -> &[EditorInput] {
        &self.editors
    }

    /// Find the index of the first editor whose URI matches `uri`.
    pub fn find_by_uri(&self, uri: &VsUri) -> Option<usize> {
        self.editors.iter().position(|e| e.uri == *uri)
    }

    /// Return the number of open editors.
    pub fn count(&self) -> usize {
        self.editors.len()
    }
}

// ---------------------------------------------------------------------------
// EditorService
// ---------------------------------------------------------------------------

/// Manages multiple [`EditorGroup`]s and tracks the active editor.
pub struct EditorService {
    groups: Vec<EditorGroup>,
    active_group: usize,
    next_group_id: u32,
    on_did_active_editor_change: Emitter<Option<EditorInput>>,
}

impl EditorService {
    /// Create a new service with one empty group.
    pub fn new() -> Self {
        Self {
            groups: vec![EditorGroup::new(0)],
            active_group: 0,
            next_group_id: 1,
            on_did_active_editor_change: Emitter::new(),
        }
    }

    /// Open an editor input in the specified group.
    ///
    /// If `group_index` is out of range the input is opened in the active group.
    pub fn open_editor(&mut self, input: EditorInput, group_index: Option<usize>) {
        let idx = group_index
            .filter(|&i| i < self.groups.len())
            .unwrap_or(self.active_group);
        self.groups[idx].open(input);
        self.active_group = idx;
        self.fire_active_change();
    }

    /// Close the editor at `index` in the group at `group_index`.
    pub fn close_editor(&mut self, group_index: usize, index: usize) {
        if let Some(group) = self.groups.get_mut(group_index) {
            group.close(index);
            self.fire_active_change();
        }
    }

    /// Return the active editor across all groups.
    pub fn get_active_editor(&self) -> Option<&EditorInput> {
        self.groups
            .get(self.active_group)
            .and_then(|g| g.active_editor())
    }

    /// Return a reference to the active group.
    pub fn get_active_group(&self) -> &EditorGroup {
        &self.groups[self.active_group]
    }

    /// Add a new empty group and return its index.
    pub fn add_group(&mut self) -> usize {
        let id = self.next_group_id;
        self.next_group_id += 1;
        self.groups.push(EditorGroup::new(id));
        self.groups.len() - 1
    }

    /// Remove the group at `index`. The active group is adjusted accordingly.
    ///
    /// At least one group must remain; removing the last group is a no-op.
    pub fn remove_group(&mut self, index: usize) {
        if self.groups.len() <= 1 || index >= self.groups.len() {
            return;
        }
        self.groups.remove(index);
        if self.active_group >= self.groups.len() {
            self.active_group = self.groups.len() - 1;
        } else if index < self.active_group {
            self.active_group -= 1;
        }
        self.fire_active_change();
    }

    /// Return a slice of all groups.
    pub fn get_groups(&self) -> &[EditorGroup] {
        &self.groups
    }

    /// Set the active group by index.
    pub fn set_active_group(&mut self, index: usize) {
        if index < self.groups.len() {
            self.active_group = index;
            self.fire_active_change();
        }
    }

    /// Event that fires when the active editor changes.
    pub fn on_did_active_editor_change(&self) -> Event<Option<EditorInput>> {
        self.on_did_active_editor_change.event()
    }

    fn fire_active_change(&self) {
        let active = self.get_active_editor().cloned();
        self.on_did_active_editor_change.fire(&active);
    }
}

impl Default for EditorService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorTab
// ---------------------------------------------------------------------------

/// Represents a single editor tab with file content and cursor state.
#[derive(Debug, Clone)]
pub struct EditorTab {
    pub id: usize,
    pub file_path: Option<PathBuf>,
    pub title: String,
    pub is_modified: bool,
    pub is_active: bool,
    pub content: String,
    pub cursor_line: u32,
    pub cursor_col: u32,
}

// ---------------------------------------------------------------------------
// EditorTabService
// ---------------------------------------------------------------------------

/// Manages a flat list of editor tabs with active-tab tracking.
pub struct EditorTabService {
    tabs: Vec<EditorTab>,
    active_tab: Option<usize>,
    next_id: usize,
}

impl EditorTabService {
    /// Create a new empty tab service.
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
            next_id: 0,
        }
    }

    /// Open a new tab, returning its id. The new tab becomes active.
    pub fn open_tab(&mut self, path: Option<PathBuf>, content: &str) -> usize {
        // If a tab with the same path is already open, activate it.
        if let Some(ref p) = path {
            if let Some(pos) = self.tabs.iter().position(|t| t.file_path.as_ref() == Some(p)) {
                self.set_active_tab_index(pos);
                return self.tabs[pos].id;
            }
        }

        let id = self.next_id;
        self.next_id += 1;

        let title = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("Untitled-{}", id));

        if let Some(active) = self.active_tab {
            if let Some(t) = self.tabs.get_mut(active) {
                t.is_active = false;
            }
        }

        let tab = EditorTab {
            id,
            file_path: path,
            title,
            is_modified: false,
            is_active: true,
            content: content.to_string(),
            cursor_line: 1,
            cursor_col: 1,
        };
        self.tabs.push(tab);
        self.active_tab = Some(self.tabs.len() - 1);
        id
    }

    /// Close a tab by id. Returns `true` if closed, `false` if dirty.
    pub fn close_tab(&mut self, id: usize) -> bool {
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return true;
        };
        if self.tabs[pos].is_modified {
            return false;
        }
        self.tabs.remove(pos);

        if self.tabs.is_empty() {
            self.active_tab = None;
        } else if let Some(active) = self.active_tab {
            if pos == active {
                let new = pos.min(self.tabs.len() - 1);
                self.active_tab = Some(new);
                self.tabs[new].is_active = true;
            } else if pos < active {
                self.active_tab = Some(active - 1);
            }
        }
        true
    }

    /// Return a reference to the active tab.
    pub fn get_active_tab(&self) -> Option<&EditorTab> {
        self.active_tab.and_then(|i| self.tabs.get(i))
    }

    /// Return a mutable reference to the active tab.
    pub fn get_active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        self.active_tab.and_then(|i| self.tabs.get_mut(i))
    }

    /// Set the active tab by id.
    pub fn set_active_tab(&mut self, id: usize) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.set_active_tab_index(pos);
        }
    }

    /// Switch to the next tab (wrapping).
    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active) = self.active_tab {
            let next = (active + 1) % self.tabs.len();
            self.set_active_tab_index(next);
        }
    }

    /// Switch to the previous tab (wrapping).
    pub fn previous_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active) = self.active_tab {
            let prev = if active == 0 {
                self.tabs.len() - 1
            } else {
                active - 1
            };
            self.set_active_tab_index(prev);
        }
    }

    /// Set the modified flag on a tab by id.
    pub fn set_modified(&mut self, id: usize, modified: bool) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.is_modified = modified;
        }
    }

    /// Return a slice of all tabs.
    pub fn get_tabs(&self) -> &[EditorTab] {
        &self.tabs
    }

    /// Return the number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Update the cursor position for a tab by id.
    pub fn update_cursor(&mut self, id: usize, line: u32, col: u32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.cursor_line = line;
            t.cursor_col = col;
        }
    }

    fn set_active_tab_index(&mut self, index: usize) {
        if let Some(old) = self.active_tab {
            if let Some(t) = self.tabs.get_mut(old) {
                t.is_active = false;
            }
        }
        self.active_tab = Some(index);
        if let Some(t) = self.tabs.get_mut(index) {
            t.is_active = true;
        }
    }
}

impl Default for EditorTabService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tab pinning and reordering
// ---------------------------------------------------------------------------

impl EditorTabService {
    /// Pin a tab by id. Pinned tabs are marked via a naming convention
    /// (title prefixed with a pin marker) and moved to the front.
    pub fn pin_tab(&mut self, id: usize) -> bool {
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        if self.tabs[pos].title.starts_with('\u{1F4CC}') {
            return false; // already pinned
        }
        self.tabs[pos].title = format!("\u{1F4CC}{}", self.tabs[pos].title);
        // Move pinned tab to front (after other pinned tabs).
        let first_unpinned = self
            .tabs
            .iter()
            .position(|t| !t.title.starts_with('\u{1F4CC}'))
            .unwrap_or(self.tabs.len());
        if pos > first_unpinned {
            let tab = self.tabs.remove(pos);
            self.tabs.insert(first_unpinned, tab);
            // Adjust active index.
            if let Some(ref mut active) = self.active_tab {
                if *active == pos {
                    *active = first_unpinned;
                } else if *active >= first_unpinned && *active < pos {
                    *active += 1;
                }
            }
        }
        true
    }

    /// Unpin a tab by id.
    pub fn unpin_tab(&mut self, id: usize) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            if tab.title.starts_with('\u{1F4CC}') {
                tab.title = tab.title.trim_start_matches('\u{1F4CC}').to_string();
                return true;
            }
        }
        false
    }

    /// Check if a tab is pinned.
    pub fn is_pinned(&self, id: usize) -> bool {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .map_or(false, |t| t.title.starts_with('\u{1F4CC}'))
    }

    /// Move a tab from one index to another.
    pub fn reorder_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        // Update active index to follow the move.
        if let Some(ref mut active) = self.active_tab {
            if *active == from {
                *active = to;
            } else if from < *active && to >= *active {
                *active -= 1;
            } else if from > *active && to <= *active {
                *active += 1;
            }
        }
        true
    }

    /// Return indices of all pinned tabs.
    pub fn pinned_tab_indices(&self) -> Vec<usize> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.title.starts_with('\u{1F4CC}'))
            .map(|(i, _)| i)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Editor group splitting statistics
// ---------------------------------------------------------------------------

/// Statistics about editor groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupStats {
    pub group_count: usize,
    pub total_tabs: usize,
    pub max_tabs_in_group: usize,
    pub empty_groups: usize,
}

impl EditorService {
    /// Compute statistics across all editor groups.
    pub fn group_stats(&self) -> GroupStats {
        let group_count = self.groups.len();
        let total_tabs: usize = self.groups.iter().map(|g| g.count()).sum();
        let max_tabs_in_group = self.groups.iter().map(|g| g.count()).max().unwrap_or(0);
        let empty_groups = self.groups.iter().filter(|g| g.count() == 0).count();
        GroupStats {
            group_count,
            total_tabs,
            max_tabs_in_group,
            empty_groups,
        }
    }
}

// ---------------------------------------------------------------------------
// EditorGroupLayout
// ---------------------------------------------------------------------------

/// Describes how editor groups are laid out in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGroupLayout {
    /// A single editor pane, no splits.
    Single,
    /// Groups are arranged side-by-side horizontally.
    Horizontal,
    /// Groups are arranged top-to-bottom vertically.
    Vertical,
}

impl EditorGroupLayout {
    /// Human-readable description of the layout.
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Single => "single pane",
            Self::Horizontal => "horizontal split",
            Self::Vertical => "vertical split",
        }
    }

    /// The minimum number of visible splits for this layout.
    pub fn split_count(&self) -> usize {
        match self {
            Self::Single => 1,
            Self::Horizontal | Self::Vertical => 2,
        }
    }

    /// Whether this layout involves a split.
    pub fn is_split(&self) -> bool {
        !matches!(self, Self::Single)
    }
}

// ---------------------------------------------------------------------------
// EditorTabReorder
// ---------------------------------------------------------------------------

/// Records a drag-to-reorder operation on tabs within a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTabReorder {
    pub from_index: usize,
    pub to_index: usize,
    pub group_id: u32,
}

impl EditorTabReorder {
    /// Create a new reorder operation. Returns `None` if `from == to`.
    pub fn new(from_index: usize, to_index: usize, group_id: u32) -> Option<Self> {
        if from_index == to_index {
            return None;
        }
        Some(Self {
            from_index,
            to_index,
            group_id,
        })
    }

    /// Validate that both indices are within `tab_count`.
    pub fn is_valid(&self, tab_count: usize) -> bool {
        self.from_index < tab_count && self.to_index < tab_count
    }

    /// Apply the reorder to a mutable slice of editor inputs.
    /// Returns `true` if the reorder was applied, `false` if indices are
    /// out of range.
    pub fn apply(&self, editors: &mut Vec<EditorInput>) -> bool {
        if !self.is_valid(editors.len()) {
            return false;
        }
        let item = editors.remove(self.from_index);
        editors.insert(self.to_index, item);
        true
    }
}

// ---------------------------------------------------------------------------
// Focus cycling
// ---------------------------------------------------------------------------

/// Given the active group index and total group count, return the next group
/// index (cycling back to 0 after the last group).
pub fn editor_group_focus_cycle(active: usize, group_count: usize) -> usize {
    if group_count == 0 {
        return 0;
    }
    (active + 1) % group_count
}

/// Like [`editor_group_focus_cycle`] but cycles in reverse.
pub fn editor_group_focus_cycle_reverse(active: usize, group_count: usize) -> usize {
    if group_count == 0 {
        return 0;
    }
    if active == 0 {
        group_count - 1
    } else {
        active - 1
    }
}

// ---------------------------------------------------------------------------
// EditorService – split & group helpers
// ---------------------------------------------------------------------------

impl EditorService {
    /// Split the active group by cloning its tab list into a new group.
    ///
    /// Returns the index of the newly created group, or `None` if the active
    /// group is empty (nothing to clone).
    pub fn split_group(&mut self) -> Option<usize> {
        let src = &self.groups[self.active_group];
        if src.count() == 0 {
            return None;
        }
        let cloned_editors: Vec<EditorInput> = src.editors.clone();
        let active_idx = src.active_index;

        let new_id = self.next_group_id;
        self.next_group_id += 1;
        let mut new_group = EditorGroup::new(new_id);
        new_group.editors = cloned_editors;
        new_group.active_index = active_idx;
        self.groups.push(new_group);
        Some(self.groups.len() - 1)
    }
}

// ---------------------------------------------------------------------------
// EditorGroup – close helpers
// ---------------------------------------------------------------------------

impl EditorGroup {
    /// Close all non-dirty editors in this group. Returns the number of
    /// editors that were closed.
    pub fn close_all_in_group(&mut self) -> usize {
        let before = self.editors.len();
        self.editors.retain(|e| e.is_dirty);
        let after = self.editors.len();

        // Fix up active index.
        if self.editors.is_empty() {
            self.active_index = None;
        } else if let Some(active) = self.active_index {
            if active >= self.editors.len() {
                self.active_index = Some(self.editors.len() - 1);
            }
        }

        before - after
    }
}

// ---------------------------------------------------------------------------
// EditorService – query helpers
// ---------------------------------------------------------------------------

impl EditorService {
    /// Return the total number of editors across all groups.
    pub fn total_editor_count(&self) -> usize {
        self.groups.iter().map(|g| g.count()).sum()
    }

    /// Return the number of editor groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Return the index of the active group.
    pub fn active_group_index(&self) -> usize {
        self.active_group
    }

    /// Search all groups for the first editor whose URI matches `uri`.
    /// Returns `(group_index, editor_index)` if found.
    pub fn find_editor_by_uri(&self, uri: &VsUri) -> Option<(usize, usize)> {
        for (gi, group) in self.groups.iter().enumerate() {
            if let Some(ei) = group.find_by_uri(uri) {
                return Some((gi, ei));
            }
        }
        None
    }
}

impl fmt::Display for EditorService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorService(groups={}, total_editors={}, active_group={})",
            self.groups.len(),
            self.total_editor_count(),
            self.active_group,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorTabService – query helpers
// ---------------------------------------------------------------------------

impl EditorTabService {
    /// Return all tabs that have unsaved changes.
    pub fn modified_tabs(&self) -> Vec<&EditorTab> {
        self.tabs.iter().filter(|t| t.is_modified).collect()
    }

    /// Find the first tab whose file path matches `path`.
    pub fn find_tab_by_path(&self, path: &PathBuf) -> Option<&EditorTab> {
        self.tabs.iter().find(|t| t.file_path.as_ref() == Some(path))
    }
}

impl fmt::Display for EditorTabService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modified = self.modified_tabs().len();
        write!(
            f,
            "EditorTabService(tabs={}, modified={}, active={:?})",
            self.tabs.len(),
            modified,
            self.active_tab,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorTab – helpers
// ---------------------------------------------------------------------------

impl EditorTab {
    /// Return the display title: the file name from the path, or the tab title.
    pub fn display_title(&self) -> &str {
        &self.title
    }
}

// ---------------------------------------------------------------------------
// EditorGroupLayout – from_count
// ---------------------------------------------------------------------------

impl EditorGroupLayout {
    /// Determine an appropriate layout from the number of groups.
    pub fn from_count(n: usize) -> Self {
        match n {
            0 | 1 => Self::Single,
            _ => Self::Horizontal,
        }
    }
}

// ---------------------------------------------------------------------------
// EditorHistory
// ---------------------------------------------------------------------------

/// Tracks recently visited editors with back/forward navigation.
#[derive(Debug)]
pub struct EditorHistory {
    entries: Vec<VsUri>,
    cursor: usize,
    capacity: usize,
}

impl EditorHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            capacity: capacity.max(1),
        }
    }

    /// Record a navigation to `uri`.
    ///
    /// Any forward history beyond the current cursor is discarded.
    /// If the URI is the same as the current entry, it is not duplicated.
    pub fn push(&mut self, uri: VsUri) {
        if self.cursor < self.entries.len() {
            if self.entries[self.cursor] == uri {
                return;
            }
        }
        // Discard forward history.
        self.entries.truncate(self.cursor + if self.entries.is_empty() { 0 } else { 1 });
        self.entries.push(uri);
        if self.entries.len() > self.capacity {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len() - 1;
    }

    /// Navigate backward. Returns the URI navigated to, if any.
    pub fn go_back(&mut self) -> Option<&VsUri> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Navigate forward. Returns the URI navigated to, if any.
    pub fn go_forward(&mut self) -> Option<&VsUri> {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Return the current entry, if any.
    pub fn current(&self) -> Option<&VsUri> {
        self.entries.get(self.cursor)
    }

    /// Whether there is a previous entry to navigate to.
    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    /// Whether there is a next entry to navigate to.
    pub fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Return the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the entire history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }
}

impl fmt::Display for EditorHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorHistory(entries={}, cursor={}, capacity={})",
            self.entries.len(),
            self.cursor,
            self.capacity,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorSnapshot
// ---------------------------------------------------------------------------

/// A serialisable snapshot of the full editor state (all groups and their tabs).
#[derive(Debug, Clone)]
pub struct EditorSnapshot {
    pub groups: Vec<EditorSnapshotGroup>,
    pub active_group: usize,
}

/// One group within an [`EditorSnapshot`].
#[derive(Debug, Clone)]
pub struct EditorSnapshotGroup {
    pub group_id: u32,
    pub editors: Vec<EditorInput>,
    pub active_index: Option<usize>,
}

impl EditorSnapshot {
    /// Capture a snapshot from the current state of an [`EditorService`].
    pub fn capture(svc: &EditorService) -> Self {
        let groups = svc
            .get_groups()
            .iter()
            .map(|g| EditorSnapshotGroup {
                group_id: g.id,
                editors: g.get_editors().to_vec(),
                active_index: g.active_editor().map(|_| {
                    g.get_editors()
                        .iter()
                        .position(|e| Some(e) == g.active_editor())
                        .unwrap_or(0)
                }),
            })
            .collect();
        Self {
            groups,
            active_group: svc.active_group_index(),
        }
    }

    /// Restore the snapshot into an [`EditorService`], replacing its state.
    pub fn restore(&self, svc: &mut EditorService) {
        svc.groups.clear();
        for sg in &self.groups {
            let mut group = EditorGroup::new(sg.group_id);
            for editor in &sg.editors {
                group.open(editor.clone());
            }
            if let Some(idx) = sg.active_index {
                group.set_active(idx);
            }
            svc.groups.push(group);
        }
        if svc.groups.is_empty() {
            svc.groups.push(EditorGroup::new(0));
        }
        svc.active_group = self.active_group.min(svc.groups.len() - 1);
        svc.next_group_id = svc
            .groups
            .iter()
            .map(|g| g.id + 1)
            .max()
            .unwrap_or(0);
    }

    /// Return the total number of editors across all groups in the snapshot.
    pub fn total_editors(&self) -> usize {
        self.groups.iter().map(|g| g.editors.len()).sum()
    }
}

impl fmt::Display for EditorSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorSnapshot(groups={}, total_editors={}, active_group={})",
            self.groups.len(),
            self.total_editors(),
            self.active_group,
        )
    }
}

impl From<&EditorService> for EditorSnapshot {
    fn from(svc: &EditorService) -> Self {
        Self::capture(svc)
    }
}

// ---------------------------------------------------------------------------
// TabSortStrategy / TabSorter
// ---------------------------------------------------------------------------

/// Strategies for sorting editor tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSortStrategy {
    /// Sort alphabetically by display name.
    ByName,
    /// Sort by full file path.
    ByPath,
    /// Sort by file extension, then by name within the same extension.
    ByExtension,
    /// Sort with modified (dirty) tabs first.
    ModifiedFirst,
}

impl fmt::Display for TabSortStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByName => write!(f, "by name"),
            Self::ByPath => write!(f, "by path"),
            Self::ByExtension => write!(f, "by extension"),
            Self::ModifiedFirst => write!(f, "modified first"),
        }
    }
}

/// Sorts a collection of [`EditorInput`]s according to a chosen strategy.
pub struct TabSorter;

impl TabSorter {
    /// Sort `editors` in place using the given `strategy`.
    pub fn sort(editors: &mut [EditorInput], strategy: TabSortStrategy) {
        match strategy {
            TabSortStrategy::ByName => {
                editors.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
            TabSortStrategy::ByPath => {
                editors.sort_by(|a, b| a.uri.path.cmp(&b.uri.path));
            }
            TabSortStrategy::ByExtension => {
                editors.sort_by(|a, b| {
                    let ext_a = Self::extension(&a.name);
                    let ext_b = Self::extension(&b.name);
                    ext_a
                        .cmp(&ext_b)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            TabSortStrategy::ModifiedFirst => {
                editors.sort_by(|a, b| b.is_dirty.cmp(&a.is_dirty).then_with(|| a.name.cmp(&b.name)));
            }
        }
    }

    /// Return a sorted copy without modifying the input.
    pub fn sorted(editors: &[EditorInput], strategy: TabSortStrategy) -> Vec<EditorInput> {
        let mut copy = editors.to_vec();
        Self::sort(&mut copy, strategy);
        copy
    }

    fn extension(name: &str) -> String {
        name.rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// EditorFilter
// ---------------------------------------------------------------------------

/// Filters editor inputs matching various criteria.
pub struct EditorFilter;

impl EditorFilter {
    /// Return editors that have unsaved changes.
    pub fn dirty(editors: &[EditorInput]) -> Vec<&EditorInput> {
        editors.iter().filter(|e| e.is_dirty).collect()
    }

    /// Return editors matching a specific language identifier.
    pub fn by_language<'a>(editors: &'a [EditorInput], lang: &str) -> Vec<&'a EditorInput> {
        editors
            .iter()
            .filter(|e| e.language_id.as_deref() == Some(lang))
            .collect()
    }

    /// Return editors whose URI path contains `pattern`.
    pub fn by_path_contains<'a>(editors: &'a [EditorInput], pattern: &str) -> Vec<&'a EditorInput> {
        editors
            .iter()
            .filter(|e| e.uri.path.contains(pattern))
            .collect()
    }

    /// Return editors whose name matches a glob-like suffix (e.g. `".rs"`).
    pub fn by_extension<'a>(editors: &'a [EditorInput], ext: &str) -> Vec<&'a EditorInput> {
        let suffix = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{ext}")
        };
        editors
            .iter()
            .filter(|e| e.name.ends_with(&suffix))
            .collect()
    }

    /// Return editors that are read-only.
    pub fn readonly(editors: &[EditorInput]) -> Vec<&EditorInput> {
        editors.iter().filter(|e| e.is_readonly).collect()
    }
}

// ---------------------------------------------------------------------------
// Display impls for remaining types
// ---------------------------------------------------------------------------

impl fmt::Display for EditorInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dirty = if self.is_dirty { " [modified]" } else { "" };
        let ro = if self.is_readonly { " [readonly]" } else { "" };
        write!(f, "{}{}{}", self.name, dirty, ro)
    }
}

impl fmt::Display for EditorGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorGroup(id={}, editors={}, active={:?})",
            self.id,
            self.editors.len(),
            self.active_index,
        )
    }
}

impl fmt::Display for EditorGroupLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.describe())
    }
}

impl fmt::Display for EditorTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modified = if self.is_modified { " [modified]" } else { "" };
        write!(f, "{}{}", self.title, modified)
    }
}

// ---------------------------------------------------------------------------
// EditorGroupLayoutSerializer
// ---------------------------------------------------------------------------

/// Serializes and deserializes [`EditorGroupLayout`] to/from a string tag.
pub struct EditorGroupLayoutSerializer;

impl EditorGroupLayoutSerializer {
    /// Encode a layout as a short string identifier.
    pub fn serialize(layout: &EditorGroupLayout) -> String {
        match layout {
            EditorGroupLayout::Single => "single".to_string(),
            EditorGroupLayout::Horizontal => "horizontal".to_string(),
            EditorGroupLayout::Vertical => "vertical".to_string(),
        }
    }

    /// Parse a string identifier back into a layout.
    pub fn deserialize(s: &str) -> Option<EditorGroupLayout> {
        match s {
            "single" => Some(EditorGroupLayout::Single),
            "horizontal" => Some(EditorGroupLayout::Horizontal),
            "vertical" => Some(EditorGroupLayout::Vertical),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// EditorTabHistory
// ---------------------------------------------------------------------------

/// Most-recently-used history of editor URIs with timestamps.
#[derive(Debug, Clone)]
pub struct EditorTabHistory {
    entries: Vec<(String, u64)>,
    capacity: usize,
}

impl EditorTabHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Push a URI to the front of the history (MRU ordering).
    ///
    /// If the URI already exists it is moved to the front with the new
    /// timestamp. The history is truncated to `capacity` afterwards.
    pub fn push(&mut self, uri: impl Into<String>, timestamp: u64) {
        let uri = uri.into();
        self.entries.retain(|(u, _)| u != &uri);
        self.entries.insert(0, (uri, timestamp));
        self.entries.truncate(self.capacity);
    }

    /// Return up to `n` most-recent entries.
    pub fn recent(&self, n: usize) -> &[(String, u64)] {
        let end = n.min(self.entries.len());
        &self.entries[..end]
    }

    /// Check whether the history contains the given URI.
    pub fn contains(&self, uri: &str) -> bool {
        self.entries.iter().any(|(u, _)| u == uri)
    }

    /// Return the most recently pushed entry, if any.
    pub fn most_recent(&self) -> Option<&(String, u64)> {
        self.entries.first()
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// EditorAutoSaveScheduler
// ---------------------------------------------------------------------------

/// Tracks pending auto-save deadlines for editor URIs.
#[derive(Debug, Clone)]
pub struct EditorAutoSaveScheduler {
    delay_ms: u64,
    pending: HashMap<String, u64>,
    enabled: bool,
}

impl EditorAutoSaveScheduler {
    /// Create a new scheduler with the given delay in milliseconds.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: HashMap::new(),
            enabled: true,
        }
    }

    /// Schedule an auto-save for the given URI at `now + delay_ms`.
    ///
    /// If the scheduler is disabled this is a no-op.
    pub fn schedule(&mut self, uri: impl Into<String>, now: u64) {
        if !self.enabled {
            return;
        }
        self.pending.insert(uri.into(), now + self.delay_ms);
    }

    /// Return and remove all URIs whose deadline is at or before `now`.
    pub fn due_entries(&mut self, now: u64) -> Vec<String> {
        let due: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, scheduled)| **scheduled <= now)
            .map(|(uri, _)| uri.clone())
            .collect();
        for uri in &due {
            self.pending.remove(uri);
        }
        due
    }

    /// Cancel a pending auto-save for the given URI.
    pub fn cancel(&mut self, uri: &str) {
        self.pending.remove(uri);
    }

    /// Number of URIs currently pending.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Enable auto-save scheduling.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable auto-save scheduling; no new entries will be accepted.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the scheduler is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ---------------------------------------------------------------------------
// EditorDiffMode
// ---------------------------------------------------------------------------

/// Represents a side-by-side (or inline) diff between two editor resources.
#[derive(Debug, Clone)]
pub struct EditorDiffMode {
    pub left_uri: String,
    pub right_uri: String,
    pub is_inline: bool,
    pub ignore_whitespace: bool,
}

impl EditorDiffMode {
    /// Create a diff between two URIs with default settings.
    pub fn new(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self {
            left_uri: left.into(),
            right_uri: right.into(),
            is_inline: false,
            ignore_whitespace: false,
        }
    }

    /// Toggle between inline and side-by-side diff views.
    pub fn toggle_inline(&mut self) {
        self.is_inline = !self.is_inline;
    }

    /// Toggle whether whitespace differences are ignored.
    pub fn toggle_whitespace(&mut self) {
        self.ignore_whitespace = !self.ignore_whitespace;
    }

    /// A short label describing the diff.
    pub fn label(&self) -> String {
        format!("{} ↔ {}", self.left_uri, self.right_uri)
    }
}

impl fmt::Display for EditorDiffMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = if self.is_inline { "inline" } else { "side-by-side" };
        let ws = if self.ignore_whitespace {
            ", ignore whitespace"
        } else {
            ""
        };
        write!(f, "{} ({style}{ws})", self.label())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// PinnedTabManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PinnedTabManager {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl PinnedTabManager {
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

impl Default for PinnedTabManager {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for PinnedTabManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "PinnedTabManager({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// ReadonlyToggleService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReadonlyToggleService {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl ReadonlyToggleService {
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

impl Default for ReadonlyToggleService {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for ReadonlyToggleService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ReadonlyToggleService({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// PinnedTabManagerSnapshot — point-in-time snapshot of PinnedTabManager state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PinnedTabManagerSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl PinnedTabManagerSnapshot {
    pub fn capture(source: &PinnedTabManager, timestamp: u64) -> Self {
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

impl fmt::Display for PinnedTabManagerSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// ReadonlyToggleServiceStats — aggregate statistics for ReadonlyToggleService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ReadonlyToggleServiceStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl ReadonlyToggleServiceStats {
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

impl fmt::Display for ReadonlyToggleServiceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// PinnedTabManagerConfig — configuration for PinnedTabManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PinnedTabManagerConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl PinnedTabManagerConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for PinnedTabManagerConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for PinnedTabManagerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ── TabOrderManager ─────────────────────────────────────────────────────

/// Manages an ordered list of tab labels with an active index.
#[derive(Debug, Clone)]
pub struct TabOrderManager {
    tabs: Vec<String>,
    active: Option<usize>,
}

impl TabOrderManager {
    pub fn new() -> Self { Self { tabs: Vec::new(), active: None } }

    pub fn insert_tab(&mut self, index: usize, label: String) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, label);
        if self.active.is_none() { self.active = Some(idx); }
    }

    pub fn remove_tab(&mut self, index: usize) -> Option<String> {
        if index >= self.tabs.len() { return None; }
        let removed = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = None;
        } else if let Some(a) = self.active {
            if a >= self.tabs.len() { self.active = Some(self.tabs.len() - 1); }
        }
        Some(removed)
    }

    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() { return false; }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        if self.active == Some(from) { self.active = Some(to); }
        true
    }

    pub fn swap_tabs(&mut self, a: usize, b: usize) -> bool {
        if a >= self.tabs.len() || b >= self.tabs.len() { return false; }
        self.tabs.swap(a, b);
        true
    }

    pub fn set_active(&mut self, index: usize) -> bool {
        if index < self.tabs.len() { self.active = Some(index); true } else { false }
    }

    pub fn active_tab(&self) -> Option<&str> { self.active.and_then(|i| self.tabs.get(i).map(|s| s.as_str())) }
    pub fn tab_at(&self, index: usize) -> Option<&str> { self.tabs.get(index).map(|s| s.as_str()) }
    pub fn count(&self) -> usize { self.tabs.len() }

    pub fn reorder(&mut self, new_order: &[usize]) -> bool {
        if new_order.len() != self.tabs.len() { return false; }
        let mut used = vec![false; self.tabs.len()];
        for &i in new_order {
            if i >= self.tabs.len() || used[i] { return false; }
            used[i] = true;
        }
        let old = self.tabs.clone();
        for (dst, &src) in new_order.iter().enumerate() {
            self.tabs[dst] = old[src].clone();
        }
        true
    }
}

// ── TabLabelFormatter ───────────────────────────────────────────────────

/// Formats tab labels with truncation and status indicators.
pub struct TabLabelFormatter;

impl TabLabelFormatter {
    /// Extract filename from a path string.
    pub fn filename_from_path(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    /// Truncate a label to max_len, adding "…" if needed.
    pub fn truncate(label: &str, max_len: usize) -> String {
        if label.len() <= max_len { return label.to_string(); }
        if max_len <= 1 { return "…".to_string(); }
        let keep = max_len - 1;
        let truncated: String = label.chars().take(keep).collect();
        format!("{}…", truncated)
    }

    /// Add a modified indicator dot.
    pub fn with_modified_indicator(label: &str, modified: bool) -> String {
        if modified { format!("● {}", label) } else { label.to_string() }
    }

    /// Add a read-only icon.
    pub fn with_readonly_icon(label: &str, readonly: bool) -> String {
        if readonly { format!("🔒 {}", label) } else { label.to_string() }
    }

    /// Disambiguate duplicate filenames by prepending the parent directory.
    pub fn disambiguate(paths: &[&str]) -> Vec<String> {
        let filenames: Vec<&str> = paths.iter().map(|p| Self::filename_from_path(p)).collect();
        paths.iter().enumerate().map(|(i, path)| {
            let name = filenames[i];
            let dups = filenames.iter().filter(|&&n| n == name).count();
            if dups > 1 {
                let parts: Vec<&str> = path.rsplitn(3, '/').collect();
                if parts.len() >= 2 { format!("{}/{}", parts[1], name) } else { name.to_string() }
            } else {
                name.to_string()
            }
        }).collect()
    }
}

// ── TabGroupSplitter ────────────────────────────────────────────────────

/// Manages splitting and merging of tab groups.
#[derive(Debug, Clone)]
pub struct TabGroupSplitter {
    groups: Vec<Vec<String>>,
}

impl TabGroupSplitter {
    pub fn new() -> Self { Self { groups: vec![Vec::new()] }  }

    pub fn split_horizontal(&mut self, group_idx: usize) -> bool {
        if group_idx >= self.groups.len() { return false; }
        self.groups.insert(group_idx + 1, Vec::new());
        true
    }

    pub fn split_vertical(&mut self, group_idx: usize) -> bool {
        self.split_horizontal(group_idx)
    }

    pub fn merge_groups(&mut self, a: usize, b: usize) -> bool {
        if a >= self.groups.len() || b >= self.groups.len() || a == b { return false; }
        let (keep, remove) = if a < b { (a, b) } else { (b, a) };
        let taken = self.groups.remove(remove);
        self.groups[keep].extend(taken);
        true
    }

    pub fn redistribute_tabs(&mut self) {
        let all: Vec<String> = self.groups.iter().flat_map(|g| g.clone()).collect();
        let n = self.groups.len();
        if n == 0 { return; }
        let chunk_size = (all.len() + n - 1) / n;
        for (i, group) in self.groups.iter_mut().enumerate() {
            group.clear();
            let start = i * chunk_size;
            let end = (start + chunk_size).min(all.len());
            if start < all.len() {
                group.extend_from_slice(&all[start..end]);
            }
        }
    }

    pub fn group_count(&self) -> usize { self.groups.len() }

    pub fn add_to_group(&mut self, group_idx: usize, tab: String) -> bool {
        if group_idx >= self.groups.len() { return false; }
        self.groups[group_idx].push(tab);
        true
    }

    pub fn tabs_in_group(&self, group_idx: usize) -> &[String] {
        self.groups.get(group_idx).map(|g| g.as_slice()).unwrap_or(&[])
    }
}


/// Configuration manager for editor_svc functionality.
pub struct EditorSvcConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl EditorSvcConfig {
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

    pub fn merge(&mut self, other: &EditorSvcConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for editor_svc operations.
pub struct EditorSvcRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl EditorSvcRateTracker {
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

/// Validation result collector for editor_svc.
pub struct EditorSvcValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl EditorSvcValidator {
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

    pub fn merge(&mut self, other: &EditorSvcValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Editor service orchestration — extended utilities (ye)
// ---------------------------------------------------------------------------

/// Metric accumulator for editor_svc operations.
#[derive(Debug, Clone)]
pub struct YeMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YeMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for editor_svc.
#[derive(Debug, Clone)]
pub struct YeRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YeRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for editor_svc lookups.
#[derive(Debug, Clone)]
pub struct YeLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YeLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for editor_svc
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEditorSvcRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEditorSvcRingBuf {
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
pub struct XaEditorSvcCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEditorSvcCounter {
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

impl Default for XaEditorSvcCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 38
// ---------------------------------------------------------------------------

/// Generic object pool `Xc38Pool<T>`.
pub struct Xc38Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc38Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc38PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc38Pool<T> {
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
    pub fn stats(&self) -> Xc38PoolStats {
        Xc38PoolStats {
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

impl<T> Default for Xc38Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc38Scheduler`.
pub struct Xc38Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc38Scheduler {
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

impl Default for Xc38Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_38 hash for the given byte slice.
pub fn xc_38_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_38 convention.
pub fn xc_38_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_73 deepening: state machine + event bus ---

/// States for the Xd73 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd73State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd73State {
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
pub struct Xd73Transition {
    pub from: Xd73State,
    pub to: Xd73State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd73StateMachine {
    current: Xd73State,
    history: Vec<Xd73Transition>,
    step_counter: usize,
}

impl Xd73StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd73State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd73State {
        self.current
    }

    pub fn history(&self) -> &[Xd73Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd73State) -> Result<Xd73State, String> {
        let allowed = match (self.current, target) {
            (Xd73State::Idle, Xd73State::Running) => true,
            (Xd73State::Running, Xd73State::Paused) => true,
            (Xd73State::Running, Xd73State::Done) => true,
            (Xd73State::Paused, Xd73State::Running) => true,
            (Xd73State::Paused, Xd73State::Done) => true,
            (Xd73State::Done, Xd73State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_73: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd73Transition {
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
            "Xd73SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd73State> {
        let prefix = "Xd73SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd73State::Idle),
            "Running" => Some(Xd73State::Running),
            "Paused" => Some(Xd73State::Paused),
            "Done" => Some(Xd73State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd73State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd73 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd73Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd73Event {
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

type Xd73HandlerFn = Box<dyn Fn(&Xd73Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd73EventBus {
    handlers: Vec<(usize, Option<String>, Xd73HandlerFn)>,
    next_id: usize,
    published: Vec<Xd73Event>,
}

impl Xd73EventBus {
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
        F: Fn(&Xd73Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd73Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd73Event) {
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

    pub fn published_events(&self) -> &[Xd73Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #91
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf91Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf91TrieNode {
    children: std::collections::HashMap<char, Xf91TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf91Trie {
    root: Xf91TrieNode,
    count: usize,
}

impl Xf91Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf91TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf91TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf91TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf91BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf91BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 37).
pub struct Xh37SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh37SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 79 as u64,
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

/// A compact bit set supporting boolean operations (variant 37).
pub struct Xh37BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh37BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 37).
pub struct Xi37Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi37Deque<T> {
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
pub struct Xi37Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi37Interval {
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

/// A simple interval tree (variant 37).
pub struct Xi37IntervalTree {
    xi_intervals: Vec<Xi37Interval>,
}

impl Xi37IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi37Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi37Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi37Interval) -> Vec<&Xi37Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi37Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi37Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi37Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi37Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi37Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi37Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 37) ---

/// Disjoint set / union-find for crate 37.
pub struct Xj37UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj37UnionFind {
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

const XJ37_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 37.
pub struct Xj37BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj37BTreeNode<K, V>>>,
    len: usize,
}

struct Xj37BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj37BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj37BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ37_BTREE_ORDER - 1
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
        let mid = XJ37_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj37BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj37BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj37BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj37BTreeNode::xj_new_leaf();
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


// --- xk_37 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk37SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk37SegmentTree {
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
pub struct Xk37DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk37DisjointIntervals {
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

    fn make_input(path: &str) -> EditorInput {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        EditorInput {
            uri: VsUri::file(path),
            name,
            is_dirty: false,
            is_readonly: false,
            language_id: None,
        }
    }

    // -- EditorGroup --------------------------------------------------------

    #[test]
    fn group_open_and_count() {
        let mut group = EditorGroup::new(0);
        assert_eq!(group.count(), 0);
        assert!(group.active_editor().is_none());

        group.open(make_input("/a.rs"));
        assert_eq!(group.count(), 1);
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/a.rs"));
    }

    #[test]
    fn group_open_multiple() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        assert_eq!(group.count(), 2);
        // Active should be the last opened.
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/b.rs"));
    }

    #[test]
    fn group_duplicate_open_focuses_existing() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        group.open(make_input("/a.rs"));
        // Should NOT create a third tab.
        assert_eq!(group.count(), 2);
        // Active should be the existing /a.rs tab (index 0).
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/a.rs"));
    }

    #[test]
    fn group_close_active_activates_next() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        group.open(make_input("/c.rs"));
        // Active is /c.rs (index 2). Close /b.rs (index 1).
        group.set_active(1);
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/b.rs"));
        group.close(1);
        // Next tab (/c.rs, now at index 1) should be active.
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/c.rs"));
    }

    #[test]
    fn group_close_last_activates_previous() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        // Active is /b.rs (index 1).
        group.close(1);
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/a.rs"));
    }

    #[test]
    fn group_close_all() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.close(0);
        assert_eq!(group.count(), 0);
        assert!(group.active_editor().is_none());
    }

    #[test]
    fn group_set_active() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        group.set_active(0);
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/a.rs"));
    }

    #[test]
    fn group_find_by_uri() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        assert_eq!(group.find_by_uri(&VsUri::file("/b.rs")), Some(1));
        assert_eq!(group.find_by_uri(&VsUri::file("/c.rs")), None);
    }

    #[test]
    fn group_get_editors() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        let editors = group.get_editors();
        assert_eq!(editors.len(), 2);
        assert_eq!(editors[0].uri, VsUri::file("/a.rs"));
        assert_eq!(editors[1].uri, VsUri::file("/b.rs"));
    }

    #[test]
    fn group_close_before_active_adjusts_index() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        group.open(make_input("/b.rs"));
        group.open(make_input("/c.rs"));
        // Active is /c.rs (index 2). Close /a.rs (index 0).
        group.close(0);
        // Active index should shift from 2 to 1.
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/c.rs"));
    }

    // -- EditorService ------------------------------------------------------

    #[test]
    fn service_new_has_one_group() {
        let svc = EditorService::new();
        assert_eq!(svc.get_groups().len(), 1);
        assert!(svc.get_active_editor().is_none());
    }

    #[test]
    fn service_open_and_get_active() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        assert_eq!(
            svc.get_active_editor().unwrap().uri,
            VsUri::file("/a.rs")
        );
    }

    #[test]
    fn service_close_editor() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/b.rs"), None);
        svc.close_editor(0, 0);
        assert_eq!(
            svc.get_active_editor().unwrap().uri,
            VsUri::file("/b.rs")
        );
    }

    #[test]
    fn service_multiple_groups() {
        let mut svc = EditorService::new();
        let g1 = svc.add_group();
        svc.open_editor(make_input("/a.rs"), Some(0));
        svc.open_editor(make_input("/b.rs"), Some(g1));
        assert_eq!(svc.get_groups().len(), 2);
        // Active group is now g1 (last opened into).
        assert_eq!(
            svc.get_active_editor().unwrap().uri,
            VsUri::file("/b.rs")
        );
        svc.set_active_group(0);
        assert_eq!(
            svc.get_active_editor().unwrap().uri,
            VsUri::file("/a.rs")
        );
    }

    #[test]
    fn service_remove_group() {
        let mut svc = EditorService::new();
        let g1 = svc.add_group();
        svc.open_editor(make_input("/a.rs"), Some(g1));
        svc.remove_group(g1);
        assert_eq!(svc.get_groups().len(), 1);
    }

    #[test]
    fn service_remove_last_group_is_noop() {
        let mut svc = EditorService::new();
        svc.remove_group(0);
        assert_eq!(svc.get_groups().len(), 1);
    }

    #[test]
    fn service_active_editor_change_event() {
        let mut svc = EditorService::new();
        let event = svc.on_did_active_editor_change();

        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &Option<EditorInput>| {
            r.lock().unwrap().push(v.clone());
        });

        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/b.rs"), None);

        let values = received.lock().unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].as_ref().unwrap().uri, VsUri::file("/a.rs"));
        assert_eq!(values[1].as_ref().unwrap().uri, VsUri::file("/b.rs"));
    }

    #[test]
    fn service_duplicate_open_across_group() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/a.rs"), None);
        assert_eq!(svc.get_active_group().count(), 1);
    }

    #[test]
    fn service_get_active_group() {
        let mut svc = EditorService::new();
        svc.add_group();
        svc.set_active_group(1);
        assert_eq!(svc.get_active_group().id, 1);
    }

    // -- EditorTabService ---------------------------------------------------

    #[test]
    fn tab_service_new_is_empty() {
        let svc = EditorTabService::new();
        assert_eq!(svc.tab_count(), 0);
        assert!(svc.get_active_tab().is_none());
    }

    #[test]
    fn tab_service_open_tab() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(Some(PathBuf::from("/a.rs")), "hello");
        assert_eq!(svc.tab_count(), 1);
        let tab = svc.get_active_tab().unwrap();
        assert_eq!(tab.id, id);
        assert_eq!(tab.title, "a.rs");
        assert_eq!(tab.content, "hello");
        assert!(tab.is_active);
    }

    #[test]
    fn tab_service_open_multiple_tabs() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        assert_eq!(svc.tab_count(), 2);
        assert_eq!(svc.get_active_tab().unwrap().title, "b.rs");
        assert!(!svc.get_tabs()[0].is_active);
    }

    #[test]
    fn tab_service_duplicate_open_activates_existing() {
        let mut svc = EditorTabService::new();
        let id1 = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        let id2 = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        assert_eq!(svc.tab_count(), 2);
        assert_eq!(id1, id2);
        assert!(svc.get_tabs()[0].is_active);
    }

    #[test]
    fn tab_service_close_tab() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        assert!(svc.close_tab(id));
        assert_eq!(svc.tab_count(), 0);
        assert!(svc.get_active_tab().is_none());
    }

    #[test]
    fn tab_service_close_dirty_tab_returns_false() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.set_modified(id, true);
        assert!(!svc.close_tab(id));
        assert_eq!(svc.tab_count(), 1);
    }

    #[test]
    fn tab_service_close_activates_next() {
        let mut svc = EditorTabService::new();
        let id_a = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        svc.open_tab(Some(PathBuf::from("/c.rs")), "c");
        svc.set_active_tab(id_a);
        assert!(svc.close_tab(id_a));
        assert_eq!(svc.get_active_tab().unwrap().title, "b.rs");
    }

    #[test]
    fn tab_service_next_tab_wraps() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        svc.next_tab();
        assert_eq!(svc.get_active_tab().unwrap().title, "a.rs");
    }

    #[test]
    fn tab_service_previous_tab_wraps() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        svc.set_active_tab(0);
        svc.previous_tab();
        assert_eq!(svc.get_active_tab().unwrap().title, "b.rs");
    }

    #[test]
    fn tab_service_update_cursor() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(None, "");
        svc.update_cursor(id, 10, 5);
        let tab = svc.get_active_tab().unwrap();
        assert_eq!(tab.cursor_line, 10);
        assert_eq!(tab.cursor_col, 5);
    }

    #[test]
    fn tab_service_untitled_tab() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(None, "content");
        let tab = svc.get_active_tab().unwrap();
        assert!(tab.title.starts_with("Untitled-"));
        assert!(tab.file_path.is_none());
        assert_eq!(tab.id, id);
    }

    #[test]
    fn tab_service_get_active_tab_mut() {
        let mut svc = EditorTabService::new();
        svc.open_tab(None, "original");
        svc.get_active_tab_mut().unwrap().content = "changed".to_string();
        assert_eq!(svc.get_active_tab().unwrap().content, "changed");
    }

    #[test]
    fn tab_service_set_active_by_id() {
        let mut svc = EditorTabService::new();
        let id_a = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        svc.set_active_tab(id_a);
        assert_eq!(svc.get_active_tab().unwrap().id, id_a);
    }

    #[test]
    fn tab_pin_and_unpin() {
        let mut svc = EditorTabService::new();
        let id = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        assert!(!svc.is_pinned(id));
        assert!(svc.pin_tab(id));
        assert!(svc.is_pinned(id));
        // Pinning again returns false.
        assert!(!svc.pin_tab(id));
        assert!(svc.unpin_tab(id));
        assert!(!svc.is_pinned(id));
    }

    #[test]
    fn tab_reorder() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        svc.open_tab(Some(PathBuf::from("/c.rs")), "c");
        assert!(svc.reorder_tab(2, 0));
        assert_eq!(svc.get_tabs()[0].title, "c.rs");
        assert_eq!(svc.get_tabs()[1].title, "a.rs");
        assert_eq!(svc.get_tabs()[2].title, "b.rs");
    }

    #[test]
    fn tab_reorder_out_of_bounds() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        assert!(!svc.reorder_tab(0, 5));
        assert!(!svc.reorder_tab(0, 0));
    }

    #[test]
    fn pinned_tab_indices() {
        let mut svc = EditorTabService::new();
        let id_a = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        let id_c = svc.open_tab(Some(PathBuf::from("/c.rs")), "c");
        svc.pin_tab(id_a);
        svc.pin_tab(id_c);
        let pinned = svc.pinned_tab_indices();
        assert_eq!(pinned.len(), 2);
    }

    #[test]
    fn group_stats_computation() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/b.rs"), None);
        let g1 = svc.add_group();
        svc.open_editor(make_input("/c.rs"), Some(g1));
        let stats = svc.group_stats();
        assert_eq!(stats.group_count, 2);
        assert_eq!(stats.total_tabs, 3);
        assert_eq!(stats.max_tabs_in_group, 2);
        assert_eq!(stats.empty_groups, 0);
    }

    #[test]
    fn group_stats_empty_groups() {
        let mut svc = EditorService::new();
        svc.add_group();
        let stats = svc.group_stats();
        assert_eq!(stats.empty_groups, 2);
        assert_eq!(stats.total_tabs, 0);
    }

    // -- EditorGroupLayout --------------------------------------------------

    #[test]
    fn layout_describe_and_split_count() {
        assert_eq!(EditorGroupLayout::Single.describe(), "single pane");
        assert_eq!(EditorGroupLayout::Horizontal.describe(), "horizontal split");
        assert_eq!(EditorGroupLayout::Vertical.describe(), "vertical split");

        assert_eq!(EditorGroupLayout::Single.split_count(), 1);
        assert_eq!(EditorGroupLayout::Horizontal.split_count(), 2);
        assert_eq!(EditorGroupLayout::Vertical.split_count(), 2);

        assert!(!EditorGroupLayout::Single.is_split());
        assert!(EditorGroupLayout::Horizontal.is_split());
        assert!(EditorGroupLayout::Vertical.is_split());
    }

    // -- EditorTabReorder ---------------------------------------------------

    #[test]
    fn tab_reorder_same_index_returns_none() {
        assert!(EditorTabReorder::new(2, 2, 0).is_none());
    }

    #[test]
    fn tab_reorder_apply() {
        let mut editors = vec![
            make_input("/a.rs"),
            make_input("/b.rs"),
            make_input("/c.rs"),
        ];
        let reorder = EditorTabReorder::new(0, 2, 0).unwrap();
        assert!(reorder.is_valid(3));
        assert!(reorder.apply(&mut editors));
        assert_eq!(editors[0].uri, VsUri::file("/b.rs"));
        assert_eq!(editors[1].uri, VsUri::file("/c.rs"));
        assert_eq!(editors[2].uri, VsUri::file("/a.rs"));
    }

    #[test]
    fn tab_reorder_invalid_indices() {
        let reorder = EditorTabReorder::new(0, 5, 0).unwrap();
        assert!(!reorder.is_valid(3));
        let mut editors = vec![make_input("/a.rs")];
        assert!(!reorder.apply(&mut editors));
    }

    // -- Focus cycling ------------------------------------------------------

    #[test]
    fn focus_cycle_wraps() {
        assert_eq!(editor_group_focus_cycle(0, 3), 1);
        assert_eq!(editor_group_focus_cycle(2, 3), 0);
        assert_eq!(editor_group_focus_cycle(0, 1), 0);
        assert_eq!(editor_group_focus_cycle(0, 0), 0);
    }

    #[test]
    fn focus_cycle_reverse_wraps() {
        assert_eq!(editor_group_focus_cycle_reverse(1, 3), 0);
        assert_eq!(editor_group_focus_cycle_reverse(0, 3), 2);
        assert_eq!(editor_group_focus_cycle_reverse(0, 1), 0);
        assert_eq!(editor_group_focus_cycle_reverse(0, 0), 0);
    }

    // -- split_group --------------------------------------------------------

    #[test]
    fn split_group_clones_tabs() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/b.rs"), None);

        let new_idx = svc.split_group().unwrap();
        assert_eq!(svc.get_groups().len(), 2);
        let new_group = &svc.get_groups()[new_idx];
        assert_eq!(new_group.count(), 2);
        assert_eq!(new_group.get_editors()[0].uri, VsUri::file("/a.rs"));
        assert_eq!(new_group.get_editors()[1].uri, VsUri::file("/b.rs"));
    }

    #[test]
    fn split_group_empty_returns_none() {
        let mut svc = EditorService::new();
        assert!(svc.split_group().is_none());
    }

    // -- close_all_in_group -------------------------------------------------

    #[test]
    fn close_all_in_group_keeps_dirty() {
        let mut group = EditorGroup::new(0);
        group.open(make_input("/a.rs"));
        let mut dirty = make_input("/b.rs");
        dirty.is_dirty = true;
        group.open(dirty);
        group.open(make_input("/c.rs"));

        let closed = group.close_all_in_group();
        assert_eq!(closed, 2);
        assert_eq!(group.count(), 1);
        assert_eq!(group.active_editor().unwrap().uri, VsUri::file("/b.rs"));
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn service_total_editor_count() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), Some(0));
        svc.open_editor(make_input("/b.rs"), Some(0));
        let g1 = svc.add_group();
        svc.open_editor(make_input("/c.rs"), Some(g1));
        assert_eq!(svc.total_editor_count(), 3);
    }

    #[test]
    fn service_group_count() {
        let mut svc = EditorService::new();
        assert_eq!(svc.group_count(), 1);
        svc.add_group();
        assert_eq!(svc.group_count(), 2);
    }

    #[test]
    fn service_active_group_index() {
        let mut svc = EditorService::new();
        assert_eq!(svc.active_group_index(), 0);
        let g1 = svc.add_group();
        svc.set_active_group(g1);
        assert_eq!(svc.active_group_index(), g1);
    }

    #[test]
    fn service_find_editor_by_uri() {
        let mut svc = EditorService::new();
        let g1 = svc.add_group();
        svc.open_editor(make_input("/a.rs"), Some(0));
        svc.open_editor(make_input("/b.rs"), Some(g1));
        assert_eq!(svc.find_editor_by_uri(&VsUri::file("/b.rs")), Some((g1, 0)));
        assert_eq!(svc.find_editor_by_uri(&VsUri::file("/a.rs")), Some((0, 0)));
        assert!(svc.find_editor_by_uri(&VsUri::file("/nope.rs")).is_none());
    }

    #[test]
    fn service_display() {
        let svc = EditorService::new();
        let s = format!("{svc}");
        assert!(s.contains("EditorService"));
        assert!(s.contains("groups=1"));
    }

    #[test]
    fn tab_service_modified_tabs() {
        let mut svc = EditorTabService::new();
        let a = svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        let c = svc.open_tab(Some(PathBuf::from("/c.rs")), "c");
        svc.set_modified(a, true);
        svc.set_modified(c, true);
        let modified = svc.modified_tabs();
        assert_eq!(modified.len(), 2);
        assert!(modified.iter().any(|t| t.title == "a.rs"));
        assert!(modified.iter().any(|t| t.title == "c.rs"));
    }

    #[test]
    fn tab_service_find_tab_by_path() {
        let mut svc = EditorTabService::new();
        svc.open_tab(Some(PathBuf::from("/a.rs")), "a");
        svc.open_tab(Some(PathBuf::from("/b.rs")), "b");
        let found = svc.find_tab_by_path(&PathBuf::from("/a.rs"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "a.rs");
        assert!(svc.find_tab_by_path(&PathBuf::from("/nope.rs")).is_none());
    }

    #[test]
    fn tab_service_display() {
        let svc = EditorTabService::new();
        let s = format!("{svc}");
        assert!(s.contains("EditorTabService"));
        assert!(s.contains("tabs=0"));
    }

    #[test]
    fn editor_tab_display_title() {
        let tab = EditorTab {
            id: 0,
            file_path: Some(PathBuf::from("/foo/bar.rs")),
            title: "bar.rs".to_string(),
            is_modified: false,
            is_active: true,
            content: String::new(),
            cursor_line: 1,
            cursor_col: 1,
        };
        assert_eq!(tab.display_title(), "bar.rs");
    }

    #[test]
    fn editor_group_layout_from_count() {
        assert_eq!(EditorGroupLayout::from_count(0), EditorGroupLayout::Single);
        assert_eq!(EditorGroupLayout::from_count(1), EditorGroupLayout::Single);
        assert_eq!(EditorGroupLayout::from_count(2), EditorGroupLayout::Horizontal);
        assert_eq!(EditorGroupLayout::from_count(5), EditorGroupLayout::Horizontal);
    }

    // -- EditorHistory ------------------------------------------------------

    #[test]
    fn history_back_forward_navigation() {
        let mut hist = EditorHistory::new(10);
        assert!(hist.is_empty());
        assert!(!hist.can_go_back());

        hist.push(VsUri::file("/a.rs"));
        hist.push(VsUri::file("/b.rs"));
        hist.push(VsUri::file("/c.rs"));
        assert_eq!(hist.len(), 3);
        assert_eq!(hist.current(), Some(&VsUri::file("/c.rs")));

        // Go back twice.
        assert!(hist.can_go_back());
        assert_eq!(hist.go_back(), Some(&VsUri::file("/b.rs")));
        assert_eq!(hist.go_back(), Some(&VsUri::file("/a.rs")));
        assert!(!hist.can_go_back());
        assert!(hist.go_back().is_none());

        // Go forward.
        assert!(hist.can_go_forward());
        assert_eq!(hist.go_forward(), Some(&VsUri::file("/b.rs")));

        // Push discards forward history.
        hist.push(VsUri::file("/d.rs"));
        assert!(!hist.can_go_forward());
        assert_eq!(hist.current(), Some(&VsUri::file("/d.rs")));
        assert_eq!(hist.len(), 3); // a, b, d
    }

    #[test]
    fn history_capacity_eviction() {
        let mut hist = EditorHistory::new(3);
        hist.push(VsUri::file("/a.rs"));
        hist.push(VsUri::file("/b.rs"));
        hist.push(VsUri::file("/c.rs"));
        hist.push(VsUri::file("/d.rs"));
        assert_eq!(hist.len(), 3);
        // /a.rs should have been evicted.
        hist.go_back();
        hist.go_back();
        assert_eq!(hist.current(), Some(&VsUri::file("/b.rs")));
    }

    // -- EditorSnapshot -----------------------------------------------------

    #[test]
    fn snapshot_capture_and_restore() {
        let mut svc = EditorService::new();
        svc.open_editor(make_input("/a.rs"), None);
        svc.open_editor(make_input("/b.rs"), None);
        let g1 = svc.add_group();
        svc.open_editor(make_input("/c.rs"), Some(g1));

        let snap = EditorSnapshot::capture(&svc);
        assert_eq!(snap.groups.len(), 2);
        assert_eq!(snap.total_editors(), 3);

        // Restore into a fresh service.
        let mut svc2 = EditorService::new();
        snap.restore(&mut svc2);
        assert_eq!(svc2.group_count(), 2);
        assert_eq!(svc2.total_editor_count(), 3);
        assert_eq!(
            svc2.get_groups()[0].get_editors()[0].uri,
            VsUri::file("/a.rs")
        );
    }

    // -- TabSorter ----------------------------------------------------------

    #[test]
    fn tab_sorter_by_extension_and_name() {
        let mut editors = vec![
            make_input("/z.py"),
            make_input("/a.rs"),
            make_input("/b.py"),
            make_input("/m.rs"),
        ];

        TabSorter::sort(&mut editors, TabSortStrategy::ByExtension);
        // .py first, then .rs; alphabetical within each extension.
        assert_eq!(editors[0].name, "b.py");
        assert_eq!(editors[1].name, "z.py");
        assert_eq!(editors[2].name, "a.rs");
        assert_eq!(editors[3].name, "m.rs");

        // ByName sort.
        TabSorter::sort(&mut editors, TabSortStrategy::ByName);
        assert_eq!(editors[0].name, "a.rs");
        assert_eq!(editors[1].name, "b.py");
    }

    // -- EditorFilter -------------------------------------------------------

    #[test]
    fn filter_dirty_and_by_language() {
        let mut editors = vec![
            make_input("/a.rs"),
            make_input("/b.py"),
            make_input("/c.rs"),
        ];
        editors[0].is_dirty = true;
        editors[2].is_dirty = true;
        editors[0].language_id = Some("rust".to_string());
        editors[1].language_id = Some("python".to_string());
        editors[2].language_id = Some("rust".to_string());

        let dirty = EditorFilter::dirty(&editors);
        assert_eq!(dirty.len(), 2);

        let rust = EditorFilter::by_language(&editors, "rust");
        assert_eq!(rust.len(), 2);
        assert_eq!(rust[0].name, "a.rs");

        let py = EditorFilter::by_extension(&editors, "py");
        assert_eq!(py.len(), 1);
        assert_eq!(py[0].name, "b.py");

        let path_match = EditorFilter::by_path_contains(&editors, "/c");
        assert_eq!(path_match.len(), 1);
    }

    // -- EditorGroupLayoutSerializer ----------------------------------------

    #[test]
    fn layout_serializer_roundtrip() {
        let layouts = [
            EditorGroupLayout::Single,
            EditorGroupLayout::Horizontal,
            EditorGroupLayout::Vertical,
        ];
        for layout in &layouts {
            let s = EditorGroupLayoutSerializer::serialize(layout);
            let parsed = EditorGroupLayoutSerializer::deserialize(&s).unwrap();
            assert_eq!(
                EditorGroupLayoutSerializer::serialize(&parsed),
                s,
            );
        }
    }

    #[test]
    fn layout_serializer_unknown_returns_none() {
        assert!(EditorGroupLayoutSerializer::deserialize("diagonal").is_none());
        assert!(EditorGroupLayoutSerializer::deserialize("").is_none());
    }

    // -- EditorTabHistory ---------------------------------------------------

    #[test]
    fn tab_history_push_and_recent() {
        let mut hist = EditorTabHistory::new(5);
        hist.push("file:///a.rs", 1);
        hist.push("file:///b.rs", 2);
        hist.push("file:///c.rs", 3);
        assert_eq!(hist.len(), 3);
        assert_eq!(hist.recent(2).len(), 2);
        assert_eq!(hist.recent(2)[0].0, "file:///c.rs");
        assert_eq!(hist.recent(2)[1].0, "file:///b.rs");
    }

    #[test]
    fn tab_history_deduplicates() {
        let mut hist = EditorTabHistory::new(5);
        hist.push("file:///a.rs", 1);
        hist.push("file:///b.rs", 2);
        hist.push("file:///a.rs", 3);
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.most_recent().unwrap().0, "file:///a.rs");
        assert_eq!(hist.most_recent().unwrap().1, 3);
    }

    #[test]
    fn tab_history_truncates_to_capacity() {
        let mut hist = EditorTabHistory::new(3);
        for i in 0..10 {
            hist.push(format!("file:///{i}.rs"), i as u64);
        }
        assert_eq!(hist.len(), 3);
        assert_eq!(hist.most_recent().unwrap().0, "file:///9.rs");
    }

    #[test]
    fn tab_history_contains_and_clear() {
        let mut hist = EditorTabHistory::new(5);
        hist.push("file:///x.rs", 1);
        assert!(hist.contains("file:///x.rs"));
        assert!(!hist.contains("file:///y.rs"));
        hist.clear();
        assert!(hist.is_empty());
        assert!(hist.most_recent().is_none());
    }

    // -- EditorAutoSaveScheduler --------------------------------------------

    #[test]
    fn autosave_schedule_and_due() {
        let mut sched = EditorAutoSaveScheduler::new(100);
        sched.schedule("file:///a.rs", 1000);
        sched.schedule("file:///b.rs", 1050);
        assert_eq!(sched.pending_count(), 2);

        let due = sched.due_entries(1100);
        assert_eq!(due.len(), 1);
        assert!(due.contains(&"file:///a.rs".to_string()));

        let due2 = sched.due_entries(1200);
        assert_eq!(due2.len(), 1);
        assert!(due2.contains(&"file:///b.rs".to_string()));
        assert_eq!(sched.pending_count(), 0);
    }

    #[test]
    fn autosave_cancel() {
        let mut sched = EditorAutoSaveScheduler::new(50);
        sched.schedule("file:///a.rs", 0);
        sched.cancel("file:///a.rs");
        assert_eq!(sched.pending_count(), 0);
        assert!(sched.due_entries(100).is_empty());
    }

    #[test]
    fn autosave_disable_prevents_schedule() {
        let mut sched = EditorAutoSaveScheduler::new(50);
        sched.disable();
        assert!(!sched.is_enabled());
        sched.schedule("file:///a.rs", 0);
        assert_eq!(sched.pending_count(), 0);
        sched.enable();
        sched.schedule("file:///a.rs", 0);
        assert_eq!(sched.pending_count(), 1);
    }

    // -- EditorDiffMode -----------------------------------------------------

    #[test]
    fn diff_mode_label() {
        let diff = EditorDiffMode::new("a.rs", "b.rs");
        assert_eq!(diff.label(), "a.rs ↔ b.rs");
    }

    #[test]
    fn diff_mode_toggles() {
        let mut diff = EditorDiffMode::new("a.rs", "b.rs");
        assert!(!diff.is_inline);
        assert!(!diff.ignore_whitespace);
        diff.toggle_inline();
        assert!(diff.is_inline);
        diff.toggle_whitespace();
        assert!(diff.ignore_whitespace);
        diff.toggle_inline();
        assert!(!diff.is_inline);
    }

    #[test]
    fn diff_mode_display() {
        let mut diff = EditorDiffMode::new("left.rs", "right.rs");
        let s = format!("{diff}");
        assert!(s.contains("side-by-side"));
        assert!(!s.contains("ignore whitespace"));

        diff.toggle_inline();
        diff.toggle_whitespace();
        let s2 = format!("{diff}");
        assert!(s2.contains("inline"));
        assert!(s2.contains("ignore whitespace"));
    }

    #[test]
    fn diff_mode_display_no_whitespace_flag() {
        let diff = EditorDiffMode::new("x", "y");
        let s = format!("{diff}");
        assert_eq!(s, "x ↔ y (side-by-side)");
    }

    #[test] fn pinnedTabManager_new() { let s = PinnedTabManager::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn pinnedTabManager_add() { let mut s = PinnedTabManager::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn pinnedTabManager_remove() { let mut s = PinnedTabManager::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn pinnedTabManager_config() { let mut s = PinnedTabManager::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn pinnedTabManager_nav() { let mut s = PinnedTabManager::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn pinnedTabManager_filter() { let mut s = PinnedTabManager::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn pinnedTabManager_display() { assert!(format!("{}", PinnedTabManager::new()).contains("PinnedTabManager")); }
    #[test] fn readonlyToggleService_new() { let s = ReadonlyToggleService::new(); assert!(s.is_empty()); }
    #[test] fn readonlyToggleService_add() { let mut s = ReadonlyToggleService::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn readonlyToggleService_active() { let mut s = ReadonlyToggleService::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn readonlyToggleService_error() { let mut s = ReadonlyToggleService::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn readonlyToggleService_rm_group() { let mut s = ReadonlyToggleService::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn readonlyToggleService_display() { assert!(format!("{}", ReadonlyToggleService::new()).contains("ReadonlyToggleService")); }


    #[test] fn pinnedTabManager_snap_capture() {
        let s = PinnedTabManager::new();
        let snap = PinnedTabManagerSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn pinnedTabManager_snap_stale() {
        let s = PinnedTabManager::new();
        let snap = PinnedTabManagerSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn pinnedTabManager_snap_diff() {
        let s = PinnedTabManager::new();
        let s1v = PinnedTabManagerSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn pinnedTabManager_snap_display() {
        let s = PinnedTabManager::new();
        let snap = PinnedTabManagerSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn readonlyToggleService_stats_record() {
        let mut st = ReadonlyToggleServiceStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn readonlyToggleService_stats_hit_ratio() {
        let mut st = ReadonlyToggleServiceStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn readonlyToggleService_stats_merge() {
        let mut a = ReadonlyToggleServiceStats::new();
        a.total_adds = 5;
        let mut b = ReadonlyToggleServiceStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn readonlyToggleService_stats_display() {
        let st = ReadonlyToggleServiceStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn pinnedTabManager_config_default() {
        let c = PinnedTabManagerConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn pinnedTabManager_config_builder() {
        let c = PinnedTabManagerConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn pinnedTabManager_config_labels() {
        let mut c = PinnedTabManagerConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn pinnedTabManager_config_cleanup_threshold() {
        let c = PinnedTabManagerConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn pinnedTabManager_config_display() {
        assert!(format!("{}", PinnedTabManagerConfig::new()).contains("Config"));
    }
    #[test] fn readonlyToggleService_stats_peaks() {
        let mut st = ReadonlyToggleServiceStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // ── TabOrderManager tests ──

    #[test]
    fn tab_order_insert_and_active() {
        let mut mgr = TabOrderManager::new();
        mgr.insert_tab(0, "a.rs".into());
        mgr.insert_tab(1, "b.rs".into());
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active_tab(), Some("a.rs"));
        assert_eq!(mgr.tab_at(1), Some("b.rs"));
    }

    #[test]
    fn tab_order_remove() {
        let mut mgr = TabOrderManager::new();
        mgr.insert_tab(0, "a.rs".into());
        mgr.insert_tab(1, "b.rs".into());
        assert_eq!(mgr.remove_tab(0), Some("a.rs".into()));
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn tab_order_move() {
        let mut mgr = TabOrderManager::new();
        mgr.insert_tab(0, "a.rs".into());
        mgr.insert_tab(1, "b.rs".into());
        mgr.insert_tab(2, "c.rs".into());
        assert!(mgr.move_tab(0, 2));
        assert_eq!(mgr.tab_at(2), Some("a.rs"));
    }

    #[test]
    fn tab_order_swap() {
        let mut mgr = TabOrderManager::new();
        mgr.insert_tab(0, "a.rs".into());
        mgr.insert_tab(1, "b.rs".into());
        assert!(mgr.swap_tabs(0, 1));
        assert_eq!(mgr.tab_at(0), Some("b.rs"));
        assert_eq!(mgr.tab_at(1), Some("a.rs"));
    }

    // ── TabLabelFormatter tests ──

    #[test]
    fn label_filename() {
        assert_eq!(TabLabelFormatter::filename_from_path("/src/main.rs"), "main.rs");
        assert_eq!(TabLabelFormatter::filename_from_path("file.txt"), "file.txt");
    }

    #[test]
    fn label_truncate() {
        assert_eq!(TabLabelFormatter::truncate("hello", 10), "hello");
        assert_eq!(TabLabelFormatter::truncate("hello world long", 6), "hello…");
    }

    #[test]
    fn label_modified_indicator() {
        assert_eq!(TabLabelFormatter::with_modified_indicator("file.rs", true), "● file.rs");
        assert_eq!(TabLabelFormatter::with_modified_indicator("file.rs", false), "file.rs");
    }

    #[test]
    fn label_disambiguate() {
        let paths = &["src/main.rs", "tests/main.rs", "lib.rs"];
        let labels = TabLabelFormatter::disambiguate(paths);
        assert_eq!(labels[0], "src/main.rs");
        assert_eq!(labels[1], "tests/main.rs");
        assert_eq!(labels[2], "lib.rs");
    }

    // ── TabGroupSplitter tests ──

    #[test]
    fn splitter_split_and_merge() {
        let mut s = TabGroupSplitter::new();
        assert_eq!(s.group_count(), 1);
        s.split_horizontal(0);
        assert_eq!(s.group_count(), 2);
        assert!(s.merge_groups(0, 1));
        assert_eq!(s.group_count(), 1);
    }

    #[test]
    fn splitter_add_and_redistribute() {
        let mut s = TabGroupSplitter::new();
        s.split_horizontal(0);
        s.add_to_group(0, "a.rs".into());
        s.add_to_group(0, "b.rs".into());
        s.add_to_group(0, "c.rs".into());
        s.add_to_group(0, "d.rs".into());
        s.redistribute_tabs();
        assert_eq!(s.tabs_in_group(0).len(), 2);
        assert_eq!(s.tabs_in_group(1).len(), 2);
    }

    #[test]
    fn splitter_merge_invalid() {
        let mut s = TabGroupSplitter::new();
        assert!(!s.merge_groups(0, 0));
        assert!(!s.merge_groups(0, 5));
    }

    #[test]
    fn editor_svc_config_new() {
        let cfg = EditorSvcConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn editor_svc_config_set_get() {
        let mut cfg = EditorSvcConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn editor_svc_config_remove() {
        let mut cfg = EditorSvcConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn editor_svc_config_keys_sorted() {
        let mut cfg = EditorSvcConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn editor_svc_config_bump_version() {
        let mut cfg = EditorSvcConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn editor_svc_config_clear() {
        let mut cfg = EditorSvcConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn editor_svc_config_merge() {
        let mut cfg1 = EditorSvcConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = EditorSvcConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn editor_svc_config_disable() {
        let mut cfg = EditorSvcConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn editor_svc_rate_tracker_empty() {
        let rt = EditorSvcRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn editor_svc_rate_tracker_record() {
        let mut rt = EditorSvcRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn editor_svc_rate_tracker_prune() {
        let mut rt = EditorSvcRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn editor_svc_validator_valid() {
        let v = EditorSvcValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn editor_svc_validator_errors() {
        let mut v = EditorSvcValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn editor_svc_validator_clear() {
        let mut v = EditorSvcValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn editor_svc_validator_merge() {
        let mut v1 = EditorSvcValidator::new();
        v1.add_error("e1");
        let mut v2 = EditorSvcValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn editor_svc_rate_tracker_clear() {
        let mut rt = EditorSvcRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn ye_metrics_empty() {
        let m = YeMetrics::new("editor_svc");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ye_metrics_record_and_mean() {
        let mut m = YeMetrics::new("editor_svc");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ye_metrics_min_max() {
        let mut m = YeMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ye_metrics_variance_and_std() {
        let mut m = YeMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn ye_metrics_percentile() {
        let mut m = YeMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn ye_metrics_merge() {
        let mut a = YeMetrics::new("a");
        a.record(1.0);
        let mut b = YeMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn ye_metrics_reset() {
        let mut m = YeMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn ye_rate_window_empty() {
        let rw = YeRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn ye_rate_window_tick_and_rate() {
        let mut rw = YeRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn ye_lru_cache_basic() {
        let mut c = YeLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn ye_lru_cache_contains_and_keys() {
        let mut c = YeLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn ye_lru_cache_remove() {
        let mut c = YeLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn ye_metrics_sum() {
        let mut m = YeMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ye_metrics_label() {
        let m = YeMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn ye_lru_cache_clear() {
        let mut c = YeLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for editor_svc
    #[test]
    fn xa_editor_svc_ring_new() {
        let rb = super::XaEditorSvcRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_editor_svc_ring_push_len() {
        let mut rb = super::XaEditorSvcRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_editor_svc_ring_wrap() {
        let mut rb = super::XaEditorSvcRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_editor_svc_ring_mean_empty() {
        let rb = super::XaEditorSvcRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_editor_svc_ring_mean_values() {
        let mut rb = super::XaEditorSvcRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_editor_svc_ring_min_max() {
        let mut rb = super::XaEditorSvcRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_editor_svc_ring_iter() {
        let mut rb = super::XaEditorSvcRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_editor_svc_counter_new() {
        let c = super::XaEditorSvcCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_svc_counter_inc() {
        let mut c = super::XaEditorSvcCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_editor_svc_counter_inc_by() {
        let mut c = super::XaEditorSvcCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_editor_svc_counter_reset() {
        let mut c = super::XaEditorSvcCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_editor_svc_counter_clear() {
        let mut c = super::XaEditorSvcCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_svc_counter_default() {
        let c = super::XaEditorSvcCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 38 ----

    #[test]
    fn xc_38_pool_new_empty() {
        let pool: super::Xc38Pool<i32> = super::Xc38Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_38_pool_release_acquire() {
        let mut pool = super::Xc38Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_38_pool_acquire_empty() {
        let mut pool: super::Xc38Pool<i32> = super::Xc38Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_38_pool_full() {
        let mut pool = super::Xc38Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_38_pool_drain() {
        let mut pool = super::Xc38Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_38_pool_stats() {
        let mut pool = super::Xc38Pool::new(8);
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
    fn xc_38_pool_clear() {
        let mut pool = super::Xc38Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_38_pool_shrink() {
        let mut pool = super::Xc38Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_38_pool_default() {
        let pool: super::Xc38Pool<String> = super::Xc38Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_38_pool_extend() {
        let mut pool = super::Xc38Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_38_pool_retain() {
        let mut pool = super::Xc38Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_38_scheduler_round_robin() {
        let mut sched = super::Xc38Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_38_scheduler_empty() {
        let mut sched = super::Xc38Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_38_scheduler_reset() {
        let mut sched = super::Xc38Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_38_scheduler_add_remove() {
        let mut sched = super::Xc38Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_38_scheduler_targets() {
        let sched = super::Xc38Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_38_hash_empty() {
        assert_eq!(super::xc_38_hash(b""), 5381);
    }

    #[test]
    fn xc_38_hash_data() {
        let h = super::xc_38_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_38_hash(b"hello"), h);
    }

    #[test]
    fn xc_38_reverse_str() {
        assert_eq!(super::xc_38_reverse("abc"), "cba");
        assert_eq!(super::xc_38_reverse(""), "");
    }


    // --- xd_73 deepening tests ---

    #[test]
    fn xd_73_sm_initial_state() {
        let sm = Xd73StateMachine::new();
        assert_eq!(sm.current_state(), Xd73State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_73_sm_valid_idle_to_running() {
        let mut sm = Xd73StateMachine::new();
        assert!(sm.transition(Xd73State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd73State::Running);
    }

    #[test]
    fn xd_73_sm_valid_running_to_paused() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        assert!(sm.transition(Xd73State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd73State::Paused);
    }

    #[test]
    fn xd_73_sm_valid_running_to_done() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        assert!(sm.transition(Xd73State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd73State::Done);
    }

    #[test]
    fn xd_73_sm_valid_paused_to_running() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        sm.transition(Xd73State::Paused).unwrap();
        assert!(sm.transition(Xd73State::Running).is_ok());
    }

    #[test]
    fn xd_73_sm_valid_done_to_idle() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        sm.transition(Xd73State::Done).unwrap();
        assert!(sm.transition(Xd73State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd73State::Idle);
    }

    #[test]
    fn xd_73_sm_invalid_idle_to_done() {
        let mut sm = Xd73StateMachine::new();
        assert!(sm.transition(Xd73State::Done).is_err());
    }

    #[test]
    fn xd_73_sm_invalid_idle_to_paused() {
        let mut sm = Xd73StateMachine::new();
        assert!(sm.transition(Xd73State::Paused).is_err());
    }

    #[test]
    fn xd_73_sm_history_tracking() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        sm.transition(Xd73State::Paused).unwrap();
        sm.transition(Xd73State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd73State::Idle);
        assert_eq!(sm.history()[0].to, Xd73State::Running);
        assert_eq!(sm.history()[1].from, Xd73State::Running);
        assert_eq!(sm.history()[2].to, Xd73State::Done);
    }

    #[test]
    fn xd_73_sm_serialize_deserialize() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd73StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd73State::Running));
    }

    #[test]
    fn xd_73_sm_deserialize_invalid() {
        assert_eq!(Xd73StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_73_sm_reset() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd73State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_73_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd73EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd73Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_73_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd73EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd73Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd73Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_73_bus_unsubscribe() {
        let mut bus = Xd73EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_73_event_kind_and_payload() {
        let e = Xd73Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd73Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_73_bus_clear_history() {
        let mut bus = Xd73EventBus::new();
        bus.publish(Xd73Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_73_sm_step_counter_increments() {
        let mut sm = Xd73StateMachine::new();
        sm.transition(Xd73State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd73State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #91 --

    #[test]
    fn xf91_trie_insert_search() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf91_trie_starts_with() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf91_trie_remove() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf91_trie_word_count() {
        let mut t = Xf91Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf91_trie_longest_prefix() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf91_trie_all_words() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf91_trie_autocomplete() {
        let mut t = Xf91Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf91_trie_empty_search() {
        let t = Xf91Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf91_bloom_add_contains() {
        let mut bf = Xf91BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf91_bloom_probably_absent() {
        let bf = Xf91BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf91_bloom_false_positive_rate() {
        let mut bf = Xf91BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf91_bloom_clear() {
        let mut bf = Xf91BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf91_bloom_union() {
        let mut a = Xf91BloomFilter::xf_new(512, 2);
        let mut b = Xf91BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf91_bloom_intersection_estimate() {
        let mut a = Xf91BloomFilter::xf_new(512, 2);
        let mut b = Xf91BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf91_bloom_union_size_mismatch() {
        let a = Xf91BloomFilter::xf_new(256, 2);
        let b = Xf91BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh37_skip_insert_contains() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh37_skip_remove() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh37_skip_len() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh37_skip_range_query() {
        let mut sl = super::Xh37SkipList::xh_new(4);
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
    fn xh37_skip_floor_ceiling() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh37_skip_rank() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh37_skip_empty() {
        let sl = super::Xh37SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh37_skip_duplicates() {
        let mut sl = super::Xh37SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh37_bitset_set_test() {
        let mut bs = super::Xh37BitSet::xh_new(256);
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
    fn xh37_bitset_clear_count() {
        let mut bs = super::Xh37BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh37_bitset_and_or_xor() {
        let mut a = super::Xh37BitSet::xh_new(128);
        let mut b = super::Xh37BitSet::xh_new(128);
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
    fn xh37_bitset_iter_ones() {
        let mut bs = super::Xh37BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh37_bitset_first_last() {
        let mut bs = super::Xh37BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh37_bitset_empty() {
        let bs = super::Xh37BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi37_deque_push_pop_back() {
        let mut dq = super::Xi37Deque::xi_new(4);
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
    fn xi37_deque_push_pop_front() {
        let mut dq = super::Xi37Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi37_deque_mixed_ops() {
        let mut dq = super::Xi37Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi37_deque_get_and_split() {
        let mut dq = super::Xi37Deque::xi_new(8);
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
    fn xi37_deque_rotate_left() {
        let mut dq = super::Xi37Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi37_deque_rotate_right() {
        let mut dq = super::Xi37Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi37_deque_grow() {
        let mut dq = super::Xi37Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi37_deque_empty() {
        let dq = super::Xi37Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi37_interval_tree_insert_query() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi37Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi37Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi37_interval_tree_overlap() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi37Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi37Interval::xi_new(12, 20));
        let q = super::Xi37Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi37_interval_tree_remove() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi37Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi37_interval_tree_gaps() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi37Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi37Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi37Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi37Interval::xi_new(8, 10));
    }

    #[test]
    fn xi37_interval_tree_merge() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi37Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi37Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi37Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi37Interval::xi_new(10, 15));
    }

    #[test]
    fn xi37_interval_tree_all() {
        let mut tree = super::Xi37IntervalTree::xi_new();
        tree.xi_insert(super::Xi37Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi37Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi37_interval_tree_empty() {
        let tree = super::Xi37IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi37_interval_tree_contains_point() {
        let iv = super::Xi37Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 37) ---

    #[test]
    fn xj_37_uf_make_and_find() {
        let mut uf = super::Xj37UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_37_uf_union_connected() {
        let mut uf = super::Xj37UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_37_uf_component_count() {
        let mut uf = super::Xj37UnionFind::xj_new();
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
    fn xj_37_uf_component_size() {
        let mut uf = super::Xj37UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_37_uf_largest_component() {
        let mut uf = super::Xj37UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_37_uf_many_elements() {
        let mut uf = super::Xj37UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_37_uf_separate_components() {
        let mut uf = super::Xj37UnionFind::xj_new();
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
    fn xj_37_uf_path_compression() {
        let mut uf = super::Xj37UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_37_bt_insert_get() {
        let mut bt = super::Xj37BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_37_bt_contains_len() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_37_bt_replace() {
        let mut bt = super::Xj37BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_37_bt_remove() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_37_bt_keys_values() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_37_bt_range() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_37_bt_min_max() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_37_bt_many_inserts() {
        let mut bt = super::Xj37BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_37 segment tree tests ---

    #[test]
    fn xk_37_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_37_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk37SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_37_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_37_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_37_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_37_st_single_element() {
        let data = vec![42];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_37_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk37SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_37_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk37SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_37 disjoint intervals tests ---

    #[test]
    fn xk_37_di_add_and_count() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_37_di_merge_overlap() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_37_di_contains() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_37_di_remove() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_37_di_covered_length() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_37_di_gaps() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_37_di_merge_adjacent() {
        let mut di = super::Xk37DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_37_di_empty() {
        let di = super::Xk37DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
