//! File-based window state persistence.
//!
//! Saves and restores window layout state (open editors, sidebar, panel) as
//! JSON, modeled after VS Code's window state management.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Persisted window layout state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowState {
    /// URI of the currently active editor.
    pub active_editor_uri: Option<String>,
    /// URIs of all open editors.
    pub open_editor_uris: Vec<String>,
    /// Whether the sidebar is visible.
    pub sidebar_visible: bool,
    /// Sidebar width in columns/pixels.
    pub sidebar_width: u32,
    /// Whether the bottom panel is visible.
    pub panel_visible: bool,
    /// Panel height in rows/pixels.
    pub panel_height: u32,
    /// Active sidebar pane identifier.
    pub active_sidebar: Option<String>,
    /// Active panel identifier.
    pub active_panel: Option<String>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            active_editor_uri: None,
            open_editor_uris: Vec::new(),
            sidebar_visible: true,
            sidebar_width: 30,
            panel_visible: false,
            panel_height: 10,
            active_sidebar: Some("explorer".to_string()),
            active_panel: None,
        }
    }
}

/// JSON file-based state service for window layout.
pub struct StateService {
    state_path: PathBuf,
    state: WindowState,
}

impl StateService {
    /// Create a new state service writing to the given path.
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
            state: WindowState::default(),
        }
    }

    /// Create an in-memory state service (no file I/O, for testing).
    pub fn in_memory() -> Self {
        Self {
            state_path: PathBuf::from(":memory:"),
            state: WindowState::default(),
        }
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &WindowState {
        &self.state
    }

    /// Get a mutable reference to the current state.
    pub fn state_mut(&mut self) -> &mut WindowState {
        &mut self.state
    }

    /// Save the current state to the JSON file.
    pub fn save_state(&self) -> std::io::Result<()> {
        if self.state_path.as_os_str() == ":memory:" {
            return Ok(());
        }
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.state_path, json)
    }

    /// Restore state from the JSON file.
    pub fn restore_state(&mut self) -> std::io::Result<()> {
        if self.state_path.as_os_str() == ":memory:" {
            return Ok(());
        }
        if !self.state_path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&self.state_path)?;
        self.state = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(())
    }

    /// Get the file path used for state persistence.
    pub fn state_path(&self) -> &Path {
        &self.state_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_state_default() {
        let state = WindowState::default();
        assert!(state.sidebar_visible);
        assert_eq!(state.sidebar_width, 30);
        assert!(!state.panel_visible);
        assert_eq!(state.active_sidebar, Some("explorer".to_string()));
        assert!(state.open_editor_uris.is_empty());
    }

    #[test]
    fn state_service_in_memory() {
        let mut svc = StateService::in_memory();
        svc.state_mut().active_editor_uri = Some("file:///main.rs".to_string());
        assert_eq!(
            svc.state().active_editor_uri,
            Some("file:///main.rs".to_string())
        );
        // save/restore on in-memory is no-op
        svc.save_state().unwrap();
        svc.restore_state().unwrap();
    }

    #[test]
    fn state_service_save_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        {
            let mut svc = StateService::new(&path);
            svc.state_mut().active_editor_uri = Some("file:///test.rs".to_string());
            svc.state_mut().open_editor_uris = vec!["file:///a.rs".into(), "file:///b.rs".into()];
            svc.state_mut().sidebar_visible = false;
            svc.state_mut().panel_visible = true;
            svc.state_mut().panel_height = 20;
            svc.save_state().unwrap();
        }

        {
            let mut svc = StateService::new(&path);
            svc.restore_state().unwrap();
            assert_eq!(
                svc.state().active_editor_uri,
                Some("file:///test.rs".to_string())
            );
            assert_eq!(svc.state().open_editor_uris.len(), 2);
            assert!(!svc.state().sidebar_visible);
            assert!(svc.state().panel_visible);
            assert_eq!(svc.state().panel_height, 20);
        }
    }

    #[test]
    fn state_service_restore_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let mut svc = StateService::new(&path);
        // Should not error, just keep defaults
        svc.restore_state().unwrap();
        assert_eq!(svc.state(), &WindowState::default());
    }

    #[test]
    fn window_state_serialization_roundtrip() {
        let state = WindowState {
            active_editor_uri: Some("file:///hello.rs".into()),
            open_editor_uris: vec!["file:///a.rs".into()],
            sidebar_visible: false,
            sidebar_width: 40,
            panel_visible: true,
            panel_height: 15,
            active_sidebar: Some("search".into()),
            active_panel: Some("terminal".into()),
        };
        let json = serde_json::to_string(&state).unwrap();
        let restored: WindowState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, restored);
    }
}
