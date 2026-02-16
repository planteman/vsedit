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

impl Storage {
    /// Get all key-value pairs.
    pub fn entries(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT key, value FROM ItemTable ORDER BY key")
            .unwrap();
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Get all values (without keys).
    pub fn values(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM ItemTable ORDER BY key")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get keys matching a prefix.
    pub fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}%", prefix);
        let mut stmt = conn
            .prepare("SELECT key FROM ItemTable WHERE key LIKE ?1 ORDER BY key")
            .unwrap();
        stmt.query_map(params![pattern], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Remove all keys matching a prefix.
    pub fn remove_prefix(&self, prefix: &str) -> StorageResult<usize> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}%", prefix);
        let count = conn.execute("DELETE FROM ItemTable WHERE key LIKE ?1", params![pattern])?;
        Ok(count)
    }

    /// Get a float value.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Set a float value.
    pub fn set_f64(&self, key: &str, value: f64) -> StorageResult<()> {
        self.set(key, &value.to_string())
    }

    /// Set a value only if the key does not already exist. Returns true if set.
    pub fn set_if_absent(&self, key: &str, value: &str) -> StorageResult<bool> {
        if self.has(key) {
            Ok(false)
        } else {
            self.set(key, value)?;
            Ok(true)
        }
    }

    /// Increment an integer value by delta, returning the new value. Initializes to delta if missing.
    pub fn increment(&self, key: &str, delta: i64) -> StorageResult<i64> {
        let current = self.get_i64(key).unwrap_or(0);
        let new_val = current + delta;
        self.set_i64(key, new_val)?;
        Ok(new_val)
    }
}

// ---------------------------------------------------------------------------
// StorageService extras
// ---------------------------------------------------------------------------

impl StorageService {
    /// Check if a key exists in a given scope.
    pub fn has(&self, key: &str, scope: StorageScope) -> bool {
        self.get(key, scope).is_some()
    }

    /// Get a boolean from a scope.
    pub fn get_bool(&self, key: &str, scope: StorageScope) -> Option<bool> {
        self.get(key, scope).map(|v| v == "true" || v == "1")
    }

    /// Set a boolean in a scope.
    pub fn set_bool(&self, key: &str, value: bool, scope: StorageScope) -> StorageResult<()> {
        self.set(key, if value { "true" } else { "false" }, scope)
    }

    /// Get an i64 from a scope.
    pub fn get_i64(&self, key: &str, scope: StorageScope) -> Option<i64> {
        self.get(key, scope).and_then(|v| v.parse().ok())
    }

    /// Set an i64 in a scope.
    pub fn set_i64(&self, key: &str, value: i64, scope: StorageScope) -> StorageResult<()> {
        self.set(key, &value.to_string(), scope)
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for StorageScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageScope::Profile => write!(f, "Profile"),
            StorageScope::Workspace => write!(f, "Workspace"),
        }
    }
}

impl std::fmt::Display for StorageTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageTarget::User => write!(f, "User"),
            StorageTarget::Machine => write!(f, "Machine"),
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

    #[test]
    fn entries_and_values() {
        let store = Storage::in_memory().unwrap();
        store.set("b", "2").unwrap();
        store.set("a", "1").unwrap();
        let entries = store.entries();
        assert_eq!(entries, vec![("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())]);
        let values = store.values();
        assert_eq!(values, vec!["1", "2"]);
    }

    #[test]
    fn keys_with_prefix() {
        let store = Storage::in_memory().unwrap();
        store.set("editor.fontSize", "14").unwrap();
        store.set("editor.tabSize", "4").unwrap();
        store.set("terminal.fontSize", "12").unwrap();
        let keys = store.keys_with_prefix("editor.");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"editor.fontSize".to_string()));
        assert!(keys.contains(&"editor.tabSize".to_string()));
    }

    #[test]
    fn remove_prefix() {
        let store = Storage::in_memory().unwrap();
        store.set("cache.a", "1").unwrap();
        store.set("cache.b", "2").unwrap();
        store.set("config.x", "3").unwrap();
        let removed = store.remove_prefix("cache.").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
        assert!(store.has("config.x"));
    }

    #[test]
    fn float_values() {
        let store = Storage::in_memory().unwrap();
        store.set_f64("pi", 3.14159).unwrap();
        let val = store.get_f64("pi").unwrap();
        assert!((val - 3.14159).abs() < 0.0001);
    }

    #[test]
    fn set_if_absent() {
        let store = Storage::in_memory().unwrap();
        assert!(store.set_if_absent("key", "first").unwrap());
        assert!(!store.set_if_absent("key", "second").unwrap());
        assert_eq!(store.get("key"), Some("first".to_string()));
    }

    #[test]
    fn increment_value() {
        let store = Storage::in_memory().unwrap();
        assert_eq!(store.increment("counter", 5).unwrap(), 5);
        assert_eq!(store.increment("counter", 3).unwrap(), 8);
        assert_eq!(store.increment("counter", -2).unwrap(), 6);
    }

    #[test]
    fn storage_service_has() {
        let global = Storage::in_memory().unwrap();
        let svc = StorageService::new(global);
        svc.set("key", "val", StorageScope::Profile).unwrap();
        assert!(svc.has("key", StorageScope::Profile));
        assert!(!svc.has("missing", StorageScope::Profile));
    }

    #[test]
    fn storage_service_bool_and_i64() {
        let global = Storage::in_memory().unwrap();
        let svc = StorageService::new(global);
        svc.set_bool("flag", true, StorageScope::Profile).unwrap();
        assert_eq!(svc.get_bool("flag", StorageScope::Profile), Some(true));
        svc.set_i64("count", 42, StorageScope::Profile).unwrap();
        assert_eq!(svc.get_i64("count", StorageScope::Profile), Some(42));
    }

    #[test]
    fn scope_display() {
        assert_eq!(format!("{}", StorageScope::Profile), "Profile");
        assert_eq!(format!("{}", StorageScope::Workspace), "Workspace");
    }

    #[test]
    fn target_display() {
        assert_eq!(format!("{}", StorageTarget::User), "User");
        assert_eq!(format!("{}", StorageTarget::Machine), "Machine");
    }

    #[test]
    fn workspace_scope_without_workspace() {
        let global = Storage::in_memory().unwrap();
        let svc = StorageService::new(global);
        assert!(svc.get("key", StorageScope::Workspace).is_none());
        assert!(svc.set("key", "val", StorageScope::Workspace).is_ok());
        assert!(svc.remove("key", StorageScope::Workspace).is_ok());
    }
}
