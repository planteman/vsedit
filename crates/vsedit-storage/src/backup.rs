//! Backup/recovery for dirty files.
//!
//! Auto-saves file content to a backup location keyed by a hash of the URI,
//! enabling crash recovery.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::StorageError;

/// Backup service that persists dirty file content for crash recovery.
pub struct BackupService {
    backup_dir: PathBuf,
    /// In-memory index: URI → backup file path.
    index: HashMap<String, PathBuf>,
}

impl BackupService {
    /// Create a new backup service writing to the given directory.
    pub fn new(backup_dir: impl Into<PathBuf>) -> Self {
        Self {
            backup_dir: backup_dir.into(),
            index: HashMap::new(),
        }
    }

    /// Create an in-memory-only backup service (for testing).
    pub fn in_memory() -> Self {
        Self {
            backup_dir: PathBuf::from(":memory:"),
            index: HashMap::new(),
        }
    }

    fn hash_uri(uri: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(uri.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn backup_path(&self, uri: &str) -> PathBuf {
        self.backup_dir.join(Self::hash_uri(uri))
    }

    /// Create a backup of content for the given URI.
    pub fn create_backup(&mut self, uri: &str, content: &str) -> Result<(), StorageError> {
        let path = self.backup_path(uri);
        if self.backup_dir.as_os_str() != ":memory:" {
            std::fs::create_dir_all(&self.backup_dir)?;
            std::fs::write(&path, content)?;
        }
        self.index.insert(uri.to_string(), path);
        Ok(())
    }

    /// Get the backed-up content for the given URI.
    pub fn get_backup(&self, uri: &str) -> Option<String> {
        let path = self.index.get(uri)?;
        if self.backup_dir.as_os_str() == ":memory:" {
            return None;
        }
        std::fs::read_to_string(path).ok()
    }

    /// Clear the backup for the given URI.
    pub fn clear_backup(&mut self, uri: &str) -> Result<(), StorageError> {
        if let Some(path) = self.index.remove(uri) {
            if self.backup_dir.as_os_str() != ":memory:" && path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    /// Check if a backup exists for the given URI.
    pub fn has_backup(&self, uri: &str) -> bool {
        self.index.contains_key(uri)
    }

    /// List all URIs that have backups.
    pub fn backed_up_uris(&self) -> Vec<&str> {
        self.index.keys().map(|s| s.as_str()).collect()
    }

    /// Get the backup directory path.
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    /// Clear all backups.
    pub fn clear_all(&mut self) -> Result<(), StorageError> {
        let uris: Vec<String> = self.index.keys().cloned().collect();
        for uri in uris {
            self.clear_backup(&uri)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_create_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let mut svc = BackupService::new(dir.path().join("backups"));
        svc.create_backup("file:///main.rs", "fn main() {}").unwrap();
        assert!(svc.has_backup("file:///main.rs"));
        assert_eq!(
            svc.get_backup("file:///main.rs"),
            Some("fn main() {}".to_string())
        );
    }

    #[test]
    fn backup_clear() {
        let dir = tempfile::tempdir().unwrap();
        let mut svc = BackupService::new(dir.path().join("backups"));
        svc.create_backup("file:///a.rs", "content").unwrap();
        svc.clear_backup("file:///a.rs").unwrap();
        assert!(!svc.has_backup("file:///a.rs"));
    }

    #[test]
    fn backup_missing_returns_none() {
        let svc = BackupService::in_memory();
        assert!(!svc.has_backup("file:///missing.rs"));
        assert_eq!(svc.get_backup("file:///missing.rs"), None);
    }

    #[test]
    fn backup_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mut svc = BackupService::new(dir.path().join("backups"));
        svc.create_backup("file:///a.rs", "v1").unwrap();
        svc.create_backup("file:///a.rs", "v2").unwrap();
        assert_eq!(svc.get_backup("file:///a.rs"), Some("v2".to_string()));
    }

    #[test]
    fn backup_list_uris() {
        let mut svc = BackupService::in_memory();
        svc.create_backup("file:///a.rs", "a").unwrap();
        svc.create_backup("file:///b.rs", "b").unwrap();
        let mut uris: Vec<&str> = svc.backed_up_uris();
        uris.sort();
        assert_eq!(uris, vec!["file:///a.rs", "file:///b.rs"]);
    }

    #[test]
    fn backup_clear_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut svc = BackupService::new(dir.path().join("backups"));
        svc.create_backup("file:///a.rs", "a").unwrap();
        svc.create_backup("file:///b.rs", "b").unwrap();
        svc.clear_all().unwrap();
        assert!(!svc.has_backup("file:///a.rs"));
        assert!(!svc.has_backup("file:///b.rs"));
    }

    #[test]
    fn backup_hash_deterministic() {
        let h1 = BackupService::hash_uri("file:///test.rs");
        let h2 = BackupService::hash_uri("file:///test.rs");
        assert_eq!(h1, h2);
        let h3 = BackupService::hash_uri("file:///other.rs");
        assert_ne!(h1, h3);
    }
}
