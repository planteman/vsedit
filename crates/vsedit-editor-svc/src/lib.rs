//! Editor group and tab management service.
//!
//! Equivalent to VS Code's `vs/workbench/services/editor/common/editorService.ts`.
//! Manages editor instances within groups (tab strips) and exposes events for
//! active-editor changes.

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
}
