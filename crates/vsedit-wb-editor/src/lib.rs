//! Editor group and tab management.

/// Layout direction for editor groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorGroupLayout {
    Horizontal,
    Vertical,
    Grid,
}

/// Metadata for a single editor tab.
#[derive(Debug, Clone)]
pub struct EditorTabInfo {
    pub uri: String,
    pub label: String,
    pub dirty: bool,
    pub pinned: bool,
    pub preview: bool,
}

/// A group of editor tabs.
#[derive(Debug, Clone)]
pub struct EditorGroup {
    pub id: u64,
    pub editors: Vec<EditorTabInfo>,
    pub active_editor: Option<usize>,
}

/// Service that manages editor groups and their tabs.
#[derive(Debug)]
pub struct EditorGroupService {
    pub groups: Vec<EditorGroup>,
    pub active_group: Option<usize>,
    pub next_id: u64,
}

impl EditorGroupService {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            active_group: None,
            next_id: 1,
        }
    }

    /// Creates a new empty editor group and returns its id.
    pub fn create_group(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.groups.push(EditorGroup {
            id,
            editors: Vec::new(),
            active_editor: None,
        });
        if self.active_group.is_none() {
            self.active_group = Some(self.groups.len() - 1);
        }
        id
    }

    /// Closes the group with the given id. Returns `true` if found and removed.
    pub fn close_group(&mut self, id: u64) -> bool {
        if let Some(pos) = self.groups.iter().position(|g| g.id == id) {
            self.groups.remove(pos);
            // Adjust active_group index after removal.
            if self.groups.is_empty() {
                self.active_group = None;
            } else if let Some(active) = self.active_group {
                if active == pos {
                    self.active_group = Some(active.min(self.groups.len() - 1));
                } else if active > pos {
                    self.active_group = Some(active - 1);
                }
            }
            true
        } else {
            false
        }
    }

    /// Opens an editor tab in the specified group.
    pub fn open_editor(&mut self, group_id: u64, uri: String, label: String) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
            // If tab already open, just activate it.
            if let Some(idx) = group.editors.iter().position(|e| e.uri == uri) {
                group.active_editor = Some(idx);
                return;
            }
            group.editors.push(EditorTabInfo {
                uri,
                label,
                dirty: false,
                pinned: false,
                preview: false,
            });
            group.active_editor = Some(group.editors.len() - 1);
        }
    }

    /// Closes the editor tab matching `uri` in the specified group.
    pub fn close_editor(&mut self, group_id: u64, uri: &str) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
            if let Some(pos) = group.editors.iter().position(|e| e.uri == uri) {
                group.editors.remove(pos);
                if group.editors.is_empty() {
                    group.active_editor = None;
                } else if let Some(active) = group.active_editor {
                    if active >= group.editors.len() {
                        group.active_editor = Some(group.editors.len() - 1);
                    }
                }
            }
        }
    }

    /// Sets the active group by id.
    pub fn set_active_group(&mut self, id: u64) {
        if let Some(pos) = self.groups.iter().position(|g| g.id == id) {
            self.active_group = Some(pos);
        }
    }

    /// Returns a reference to the active editor group, if any.
    pub fn get_active_group(&self) -> Option<&EditorGroup> {
        self.active_group.and_then(|i| self.groups.get(i))
    }

    /// Returns the number of editor groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

impl Default for EditorGroupService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_close_groups() {
        let mut svc = EditorGroupService::new();
        assert_eq!(svc.group_count(), 0);

        let id1 = svc.create_group();
        let id2 = svc.create_group();
        assert_eq!(svc.group_count(), 2);

        assert!(svc.close_group(id1));
        assert_eq!(svc.group_count(), 1);
        assert_eq!(svc.get_active_group().unwrap().id, id2);

        assert!(!svc.close_group(999));
    }

    #[test]
    fn open_and_close_editors() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();

        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());
        svc.open_editor(gid, "file:///b.rs".into(), "b.rs".into());

        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 2);
        assert_eq!(group.active_editor, Some(1));

        // Re-opening existing tab just activates it.
        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());
        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 2);
        assert_eq!(group.active_editor, Some(0));

        svc.close_editor(gid, "file:///a.rs");
        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 1);
        assert_eq!(group.editors[0].uri, "file:///b.rs");
    }

    #[test]
    fn set_active_group() {
        let mut svc = EditorGroupService::new();
        let id1 = svc.create_group();
        let id2 = svc.create_group();

        assert_eq!(svc.get_active_group().unwrap().id, id1);
        svc.set_active_group(id2);
        assert_eq!(svc.get_active_group().unwrap().id, id2);
    }

    #[test]
    fn layout_enum_clone() {
        let layout = EditorGroupLayout::Grid;
        let cloned = layout.clone();
        assert_eq!(layout, cloned);
    }
}
