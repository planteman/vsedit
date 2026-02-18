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

}
