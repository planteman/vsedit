//! Persistent key-value storage.
//!
//! Equivalent to VS Code's `vs/platform/storage/common/storage.ts`.
//! Uses SQLite for persistent storage, with scopes for global/workspace data.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

/// Storage scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageScope {
    /// Global storage (shared across all workspaces).
    Profile,
    /// Workspace-specific storage.
    Workspace,
}

/// Storage target (where the data should be persisted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTarget {
    /// User settings storage.
    User,
    /// Machine-specific storage.
    Machine,
}

/// Error type for storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// A key-value store backed by SQLite.
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    /// Open storage from a file path, creating the DB if needed.
    pub fn open(path: &Path) -> StorageResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ItemTable (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory storage (for testing).
    pub fn in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ItemTable (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM ItemTable WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok()
    }

    /// Get a value or return a default.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    /// Get a boolean value.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).map(|v| v == "true" || v == "1")
    }

    /// Get an integer value.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Set a value.
    pub fn set(&self, key: &str, value: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO ItemTable (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Set a boolean value.
    pub fn set_bool(&self, key: &str, value: bool) -> StorageResult<()> {
        self.set(key, if value { "true" } else { "false" })
    }

    /// Set an integer value.
    pub fn set_i64(&self, key: &str, value: i64) -> StorageResult<()> {
        self.set(key, &value.to_string())
    }

    /// Remove a key.
    pub fn remove(&self, key: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ItemTable WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Check if a key exists.
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Get all keys.
    pub fn keys(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT key FROM ItemTable ORDER BY key")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get the number of stored items.
    pub fn len(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM ItemTable", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all data.
    pub fn clear(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ItemTable", [])?;
        Ok(())
    }
}

/// Manages global and workspace storage instances.
pub struct StorageService {
    global: Storage,
    workspace: Option<Storage>,
}

impl StorageService {
    pub fn new(global: Storage) -> Self {
        Self {
            global,
            workspace: None,
        }
    }

    pub fn with_workspace(mut self, workspace: Storage) -> Self {
        self.workspace = Some(workspace);
        self
    }

    pub fn get(&self, key: &str, scope: StorageScope) -> Option<String> {
        match scope {
            StorageScope::Profile => self.global.get(key),
            StorageScope::Workspace => self.workspace.as_ref().and_then(|w| w.get(key)),
        }
    }

    pub fn set(&self, key: &str, value: &str, scope: StorageScope) -> StorageResult<()> {
        match scope {
            StorageScope::Profile => self.global.set(key, value),
            StorageScope::Workspace => {
                if let Some(w) = &self.workspace {
                    w.set(key, value)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn remove(&self, key: &str, scope: StorageScope) -> StorageResult<()> {
        match scope {
            StorageScope::Profile => self.global.remove(key),
            StorageScope::Workspace => {
                if let Some(w) = &self.workspace {
                    w.remove(key)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_get_set() {
        let store = Storage::in_memory().unwrap();
        assert!(store.get("key").is_none());

        store.set("key", "value").unwrap();
        assert_eq!(store.get("key"), Some("value".to_string()));
    }

    #[test]
    fn get_or_default() {
        let store = Storage::in_memory().unwrap();
        assert_eq!(store.get_or("missing", "default"), "default");
        store.set("missing", "found").unwrap();
        assert_eq!(store.get_or("missing", "default"), "found");
    }

    #[test]
    fn bool_values() {
        let store = Storage::in_memory().unwrap();
        store.set_bool("flag", true).unwrap();
        assert_eq!(store.get_bool("flag"), Some(true));
        store.set_bool("flag", false).unwrap();
        assert_eq!(store.get_bool("flag"), Some(false));
    }

    #[test]
    fn integer_values() {
        let store = Storage::in_memory().unwrap();
        store.set_i64("count", 42).unwrap();
        assert_eq!(store.get_i64("count"), Some(42));
    }

    #[test]
    fn remove_and_has() {
        let store = Storage::in_memory().unwrap();
        store.set("key", "val").unwrap();
        assert!(store.has("key"));
        store.remove("key").unwrap();
        assert!(!store.has("key"));
    }

    #[test]
    fn keys_and_len() {
        let store = Storage::in_memory().unwrap();
        store.set("b", "1").unwrap();
        store.set("a", "2").unwrap();
        assert_eq!(store.len(), 2);
        assert_eq!(store.keys(), vec!["a", "b"]);
    }

    #[test]
    fn clear() {
        let store = Storage::in_memory().unwrap();
        store.set("a", "1").unwrap();
        store.set("b", "2").unwrap();
        store.clear().unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn overwrite_value() {
        let store = Storage::in_memory().unwrap();
        store.set("key", "v1").unwrap();
        store.set("key", "v2").unwrap();
        assert_eq!(store.get("key"), Some("v2".to_string()));
    }

    #[test]
    fn storage_service_scopes() {
        let global = Storage::in_memory().unwrap();
        let workspace = Storage::in_memory().unwrap();
        let svc = StorageService::new(global).with_workspace(workspace);

        svc.set("key", "global_val", StorageScope::Profile).unwrap();
        svc.set("key", "ws_val", StorageScope::Workspace).unwrap();

        assert_eq!(
            svc.get("key", StorageScope::Profile),
            Some("global_val".to_string())
        );
        assert_eq!(
            svc.get("key", StorageScope::Workspace),
            Some("ws_val".to_string())
        );
    }

    #[test]
    fn persistent_storage() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");

        {
            let store = Storage::open(&db_path).unwrap();
            store.set("persist", "data").unwrap();
        }

        {
            let store = Storage::open(&db_path).unwrap();
            assert_eq!(store.get("persist"), Some("data".to_string()));
        }
    }
}
