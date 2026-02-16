//! Editor group and tab management service.
//!
//! Equivalent to VS Code's `vs/workbench/services/editor/common/editorService.ts`.
//! Manages editor instances within groups (tab strips) and exposes events for
//! active-editor changes.

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
// Tests
// ---------------------------------------------------------------------------

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
}
