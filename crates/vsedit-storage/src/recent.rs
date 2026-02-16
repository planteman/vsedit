//! Recently opened files and workspaces.
//!
//! Tracks recently opened files and workspace folders, stored in global
//! storage (SQLite). Modeled after VS Code's recently opened list.

use serde::{Deserialize, Serialize};

use crate::{Storage, StorageResult};

const RECENT_KEY: &str = "recently_opened";
const MAX_RECENT: usize = 50;

/// A recently opened file entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentFileEntry {
    /// File URI or path.
    pub uri: String,
    /// Optional label for display.
    pub label: Option<String>,
}

/// A recently opened workspace entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentWorkspaceEntry {
    /// Workspace folder URI or path.
    pub uri: String,
    /// Optional label for display.
    pub label: Option<String>,
}

/// Recently opened files and workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecentlyOpened {
    pub workspaces: Vec<RecentWorkspaceEntry>,
    pub files: Vec<RecentFileEntry>,
}

/// Service for managing recently opened items, backed by [`Storage`].
pub struct RecentlyOpenedService<'a> {
    storage: &'a Storage,
}

impl<'a> RecentlyOpenedService<'a> {
    /// Create a new service backed by the given storage.
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// Load recently opened data from storage.
    pub fn get_recently_opened(&self) -> RecentlyOpened {
        self.storage
            .get(RECENT_KEY)
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save(&self, recent: &RecentlyOpened) -> StorageResult<()> {
        let json = serde_json::to_string(recent).map_err(|e| {
            crate::StorageError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        self.storage.set(RECENT_KEY, &json)
    }

    /// Add a recently opened file. Moves it to the front if already present.
    pub fn add_recent_file(&self, uri: impl Into<String>) -> StorageResult<()> {
        let uri = uri.into();
        let mut recent = self.get_recently_opened();
        recent.files.retain(|f| f.uri != uri);
        recent.files.insert(
            0,
            RecentFileEntry {
                uri,
                label: None,
            },
        );
        recent.files.truncate(MAX_RECENT);
        self.save(&recent)
    }

    /// Add a recently opened workspace. Moves it to the front if already present.
    pub fn add_recent_workspace(&self, uri: impl Into<String>) -> StorageResult<()> {
        let uri = uri.into();
        let mut recent = self.get_recently_opened();
        recent.workspaces.retain(|w| w.uri != uri);
        recent.workspaces.insert(
            0,
            RecentWorkspaceEntry {
                uri,
                label: None,
            },
        );
        recent.workspaces.truncate(MAX_RECENT);
        self.save(&recent)
    }

    /// Clear all recently opened items.
    pub fn clear_recently_opened(&self) -> StorageResult<()> {
        self.storage.remove(RECENT_KEY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recently_opened_empty_by_default() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        let recent = svc.get_recently_opened();
        assert!(recent.files.is_empty());
        assert!(recent.workspaces.is_empty());
    }

    #[test]
    fn add_recent_file() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        svc.add_recent_file("file:///a.rs").unwrap();
        svc.add_recent_file("file:///b.rs").unwrap();
        let recent = svc.get_recently_opened();
        assert_eq!(recent.files.len(), 2);
        assert_eq!(recent.files[0].uri, "file:///b.rs");
        assert_eq!(recent.files[1].uri, "file:///a.rs");
    }

    #[test]
    fn add_recent_file_deduplicates() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        svc.add_recent_file("file:///a.rs").unwrap();
        svc.add_recent_file("file:///b.rs").unwrap();
        svc.add_recent_file("file:///a.rs").unwrap();
        let recent = svc.get_recently_opened();
        assert_eq!(recent.files.len(), 2);
        assert_eq!(recent.files[0].uri, "file:///a.rs");
    }

    #[test]
    fn add_recent_workspace() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        svc.add_recent_workspace("/home/user/project").unwrap();
        let recent = svc.get_recently_opened();
        assert_eq!(recent.workspaces.len(), 1);
        assert_eq!(recent.workspaces[0].uri, "/home/user/project");
    }

    #[test]
    fn clear_recently_opened() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        svc.add_recent_file("file:///a.rs").unwrap();
        svc.add_recent_workspace("/project").unwrap();
        svc.clear_recently_opened().unwrap();
        let recent = svc.get_recently_opened();
        assert!(recent.files.is_empty());
        assert!(recent.workspaces.is_empty());
    }

    #[test]
    fn recently_opened_max_limit() {
        let storage = Storage::in_memory().unwrap();
        let svc = RecentlyOpenedService::new(&storage);
        for i in 0..60 {
            svc.add_recent_file(format!("file:///f{i}.rs")).unwrap();
        }
        let recent = svc.get_recently_opened();
        assert_eq!(recent.files.len(), MAX_RECENT);
        // Most recent should be first
        assert_eq!(recent.files[0].uri, "file:///f59.rs");
    }
}
